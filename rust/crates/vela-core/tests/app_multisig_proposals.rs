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
use world::{
    fixture, genesis, settle, settle_reads_first, DeployedSafe, Landing, World, HANDLER_141,
    SERVICE,
};

type Sut = DomainDriver<MultisigProposals>;

/// An `eth_simulateV1` answer that reads: one call, nothing moved (D1: a
/// review with no check read at all is signed only "… anyway").
const SIM_READ: &str = r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#;

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
    // A shell's clock: a bound (the look's cap) runs out after the reads it bounds.
    let answered = settle_reads_first(sut, world, 20_000);
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
        // The outbox's wait (not the core's bound on a review's question,
        // r14-13, which this order of answers can reach first).
        if matches!(
            op,
            vela_core::app::multisig_shell::MultisigOperation::Wait { ms }
                if ms != vela_core::app::multisig_gather::REVIEW_ASK_MS
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

/// Round 14 (r14-9, §20 left open): a look older than `PREVIEW_FRESH_MS`
/// was quoted again by Execute and THAT quote signed — the fee line said
/// ≈0.098172 and 0.0972 was charged. The fee signed is the fee shown: a new
/// quote whose fee differs from the look on screen signs nothing; the
/// execution fails with `fee_moved` (the new figure, its network and
/// executor), the new look is kept for the sheet, and the next press, inside
/// the window, signs exactly it.
#[test]
fn an_execution_never_signs_a_fee_other_than_the_one_shown() {
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
    let preview = Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
    };
    run(&mut sut, &mut world, preview);
    let shown = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_eq!(shown["kind"], "priced", "{shown}");
    // The press comes after the window, and the network's price moved.
    world.now += 20_000.0;
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "quoted", "fee": {"token": null, "amount": "0x2"}, "totalGas": "0x1", "executorDeployed": true}),
        },
    );
    let passkeys = world.passkeys.len();
    let execute = Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
        shown_fee: None,
    };
    run(&mut sut, &mut world, execute.clone());
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{hash}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    let detail: Value = serde_json::from_str(&detail).expect("a failure the shells read");
    assert_eq!(detail["kind"], "fee_moved");
    assert_eq!(detail["amount"], "2");
    assert_eq!(detail["chainId"], 100);
    assert!(world.sent.is_empty(), "nothing signed or sent");
    assert_eq!(world.passkeys.len(), passkeys);
    // The sheet now shows the new look; the next press signs exactly it.
    let now = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_ne!(now["fee"], shown["fee"], "{now}");
    run(&mut sut, &mut world, execute);
    assert_eq!(world.sent.len(), 1, "signed at the fee now shown");
}

/// Round 15 (desktop, not concluded in R15): a sheet re-asks its look every
/// few seconds, so a fresh look (inside `PREVIEW_FRESH_MS`) can replace the
/// fee between the paint and the press — the press signed the look's new
/// figure, not the one the person saw. The press now carries the figure
/// drawn under it (`shown_fee`): any look whose fee is another — fresh or
/// re-quoted — stops with `fee_moved`, nothing signed; a press on the new
/// figure signs exactly it.
#[test]
fn a_press_signs_only_the_fee_it_carries_even_on_a_fresh_look() {
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
    let preview = Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
    };
    // No clock runs between the look and the presses: every press is
    // inside the look's window (no re-quote of its own).
    let no_clock = |sut: &mut Sut, world: &mut World, event: Event| {
        sut.dispatch(event);
        for _ in 0..20_000 {
            let Some(op) = sut.outstanding().into_iter().find(|op| {
                !matches!(
                    op,
                    vela_core::app::multisig_shell::MultisigOperation::Wait { .. }
                )
            }) else {
                return;
            };
            let res = world.answer(&op);
            sut.resolve_matching(|o| *o == op, res);
        }
        panic!("the machine never settled");
    };
    world.now += 60_000.0;
    no_clock(&mut sut, &mut world, preview);
    let look = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_eq!(look["kind"], "priced", "{look}");
    let fee = |v: &Value| {
        vela_core::app::multisig_shell::quantity(v["fee"]["amount"].as_str().unwrap())
            .unwrap()
            .to_string()
    };
    // The person saw another figure (the look was replaced after the paint).
    let press = |shown: String| Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
        shown_fee: Some(shown),
    };
    let passkeys = world.passkeys.len();
    no_clock(&mut sut, &mut world, press("999".to_owned()));
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{hash}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    let detail: Value = serde_json::from_str(&detail).expect("a failure the shells read");
    assert_eq!(detail["kind"], "fee_moved");
    assert_eq!(
        detail["amount"],
        fee(&look),
        "the figure it would have signed"
    );
    assert!(world.sent.is_empty(), "nothing signed or sent");
    assert_eq!(world.passkeys.len(), passkeys);
    // Pressed on the figure now shown: signed at exactly it.
    no_clock(&mut sut, &mut world, press(fee(&look)));
    assert_eq!(world.sent.len(), 1, "signed at the fee shown");
}

/// Round 16 (r16-1 desktop, r16-7 iOS): after a fee-moved stop the footer
/// kept re-quoting (0.10692, then 0.11975) while the stop still said
/// "Network fee ≈ 0.099144" — two figures, and the next press signed the
/// footer's. The rule: while the stop is shown it names the figure the next
/// press signs — the executor's look now. It follows every re-quote; a look
/// with no figure withdraws it (the fee line says why nothing can run).
#[test]
fn a_fee_moved_stop_names_the_figure_the_next_press_signs() {
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
    let preview = || Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
    };
    let quote = |amount: &str| vela_core::app::multisig_shell::MultisigShellResult::Quoted {
        quote: serde_json::json!({"kind": "quoted", "fee": {"token": null, "amount": amount}, "totalGas": "0x1", "executorDeployed": true}),
    };
    world.now += 60_000.0;
    world.quote_answer = Some(quote("0x100"));
    run(&mut sut, &mut world, preview());
    let key = format!("execute:{hash}");
    let stop = |sut: &Sut| -> Option<Value> {
        match sut.view().actions.get(&key) {
            Some(ActionState::Failed { detail }) => serde_json::from_str(detail).ok(),
            None => None,
            other => panic!("{other:?}"),
        }
    };
    // Pressed on a figure that moved: stopped at the look's (256).
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
            shown_fee: Some("255".to_owned()),
        },
    );
    let said = stop(&sut).expect("the stop is said");
    assert_eq!(said["kind"], "fee_moved");
    assert_eq!(said["amount"], "256");
    assert!(world.sent.is_empty());
    // The footer re-quotes while the stop is shown: the stop follows it.
    world.now += 20_000.0;
    world.quote_answer = Some(quote("0x200"));
    run(&mut sut, &mut world, preview());
    let look = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_eq!(look["kind"], "priced", "{look}");
    let said = stop(&sut).expect("still said");
    assert_eq!(said["kind"], "fee_moved");
    assert_eq!(said["amount"], "512", "the stop names the footer's figure");
    // The next press, on that figure, signs exactly it.
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
            shown_fee: Some("512".to_owned()),
        },
    );
    assert_eq!(world.sent.len(), 1, "signed at the figure the stop named");
}

/// r16-1, the other half: a stop whose look loses its figure (the fee
/// could not be worked out) is withdrawn — it never names a figure no press
/// would sign; the fee line says why nothing can run.
#[test]
fn a_fee_moved_stop_is_withdrawn_when_the_look_has_no_figure() {
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
    let preview = || Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
    };
    world.now += 60_000.0;
    run(&mut sut, &mut world, preview());
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
            shown_fee: Some("999".to_owned()),
        },
    );
    let key = format!("execute:{hash}");
    assert!(matches!(
        sut.view().actions.get(&key),
        Some(ActionState::Failed { .. })
    ));
    world.now += 20_000.0;
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::QuoteFailed {
            detail: "no quote".to_owned(),
        },
    );
    run(&mut sut, &mut world, preview());
    let look = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_ne!(look["kind"], "priced", "{look}");
    assert!(
        !sut.view().actions.contains_key(&key),
        "no stop names a figure no press would sign: {:?}",
        sut.view().actions.get(&key)
    );
}

/// Round 14 (r14-4): with the network refusing, the desktop said "Vela won't
/// use this multisig — its setup could give control of its funds to someone
/// other than its members" of a multisig Vela created: the deploy gate's
/// read failed (`Authority::Unknown`), and the look handed it on as
/// `refused`. A setup that could not be checked is its own look —
/// `unverified`, never a finding — and a setup checked and refused stays
/// `refused`.
#[test]
fn a_deploy_whose_safe_contracts_went_unread_is_unverified_never_refused() {
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
    world.rpc_hooks.push(Box::new(|_, method, _| {
        (method == "eth_getCode").then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "connection refused".to_owned(),
            }
        })
    }));
    run(
        &mut sut,
        &mut world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    let preview = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_eq!(preview["kind"], "unverified", "{preview}");
    assert_eq!(preview["authority"]["kind"], "unknown", "{preview}");
    // Read again (the code answers wrong now): checked, and refused.
    world.rpc_hooks.clear();
    world.rpc_hooks.push(Box::new(|_, method, _| {
        (method == "eth_getCode").then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!("0x"),
            }
        })
    }));
    world.now += 120_000.0;
    run(
        &mut sut,
        &mut world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    let preview = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
    assert_eq!(preview["kind"], "refused", "{preview}");
    assert_eq!(preview["authority"]["kind"], "refused", "{preview}");
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
    let preview = sut.view().previews[&look_key(&hash, &fixture(0).address)].clone();
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
            shown_fee: None,
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
    // One check of two reads (D1): the page is told plainly.
    world.sim_reply = Some(SIM_READ.to_owned());
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
    let shown_fee = shown_fee_of(&mut sut, &mut world, &hash, &fixture(0).address);
    let passkeys = world.passkeys.len();
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
            shown_fee,
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
        owners,
        anyway,
    })) = world.purposes.first().cloned()
    else {
        panic!("{:?}", world.purposes);
    };
    // D1: a review one of whose checks read signs plainly.
    assert!(!anyway);
    assert_eq!(multisig, account.address);
    assert_eq!(name.as_deref(), Some("Quiet Harbor"));
    assert_eq!(threshold, 2);
    // r10 page: its members now, for a member change's before → after.
    assert!(
        owners
            .iter()
            .any(|o| o.eq_ignore_ascii_case(&fixture(0).address)),
        "{owners:?}"
    );
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

/// r9 (desktop): a balance read nobody answered — or answered with no
/// amount at all — is "couldn't check", never "the multisig holds 0".
#[test]
fn an_unread_balance_is_unchecked_never_zero() {
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
    let safe = account.address.to_ascii_lowercase();
    for answer in [
        vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
            result: json!("0x"),
        },
        vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
            detail: "not connected".into(),
        },
    ] {
        let safe = safe.clone();
        world.rpc_hooks.clear();
        world.rpc_hooks.push(Box::new(move |_, method, params| {
            (method == "eth_getBalance" && params[0].as_str()?.eq_ignore_ascii_case(&safe))
                .then(|| answer.clone())
        }));
        run(
            &mut sut,
            &mut world,
            Event::Execute {
                chain_id: 100,
                safe_tx_hash: hash.clone(),
                executor: fixture(0).address,
                shown_fee: None,
            },
        );
        let state = sut.view().actions[&format!("execute:{hash}")].clone();
        let said = serde_json::to_value(&state).unwrap();
        assert!(
            !said.to_string().contains("\"short\""),
            "never \"holds 0\": {said}"
        );
    }
    assert!(world.sent.is_empty());
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
            shown_fee: None,
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
    sut.view().previews[&look_key(hash, executor)].clone()
}

/// One executor's look in the view: `<chainId>:<hash>:<executor>`, lowercase.
fn look_key(hash: &str, executor: &str) -> String {
    format!(
        "100:{}:{}",
        hash.to_ascii_lowercase(),
        executor.to_ascii_lowercase()
    )
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
    // r7-8: each member is said once — the Vela account under "approves in
    // Vela", the one nothing says on its own "either way" line, never both;
    // the outside link never names a Vela account.
    let has = |list: &[String], who: &str| list.iter().any(|x| x.eq_ignore_ascii_case(who));
    assert!(has(&signers.in_vela, VELA_ON_BASE));
    assert!(!has(&signers.in_vela, PLAIN), "{signers:?}");
    assert!(has(&signers.either, PLAIN));
    assert!(!has(&signers.outside, VELA_ON_BASE));
    assert!(has(&signers.contract, OTHER_SAFE));
    assert!(!signers.enough);
}

/// r7-8: an outside member the person said is a wallet is never listed as
/// approving in Vela, and a Vela member the person said is one is never in
/// the outside-wallet link.
#[test]
fn a_kept_kind_puts_each_member_on_one_side() {
    const EVE: &str = "0x00000000000000000000000000000000000000e5";
    const BOB: &str = "0x00000000000000000000000000000000000000b0";
    let me = fixture(0).address;
    let g = genesis(&[&me, BOB, EVE], 2, "0x42");
    let mut row = world::stored_row(&g, &[100], true);
    row["members"] = json!({
        BOB: {"name": "Bob", "kind": "passkey"},
        EVE: {"name": "Eve", "kind": "wallet"},
    });
    let account: MultisigAccount = serde_json::from_value(row).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.deploy(100, &g.address, DeployedSafe::clean(&[&me, BOB, EVE], 2));
    let mut sut = Sut::new();
    let ev = start(&world, vec![100]);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account),
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
    let has = |list: &[String], who: &str| list.iter().any(|x| x.eq_ignore_ascii_case(who));
    assert!(has(&signers.in_vela, BOB), "{signers:?}");
    assert!(!has(&signers.outside, BOB), "{signers:?}");
    assert!(has(&signers.outside, EVE), "{signers:?}");
    assert!(!has(&signers.in_vela, EVE), "{signers:?}");
    assert!(signers.either.is_empty(), "{signers:?}");
}

// -- W8-M2b: a blind review, each executor's look, frozen facts, a used number, a reload --

const OTHER_TX: &str = "0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef";

fn owners_of_setup() -> Vec<String> {
    (0..3).map(|i| fixture(i).address).collect()
}

/// The multisig after its first execution: deployed, its number 1.
fn after_first(world: &mut World, account: &MultisigAccount) {
    let owners = owners_of_setup();
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.deploy(100, &account.address, after);
}

/// The figure a sheet draws for `executor`'s execution of `hash` — its look's
/// fee, as every shell carries it on the press (§24; round 16 leftover 1: a
/// press over no figure signs nothing).
fn shown_fee_of(sut: &mut Sut, world: &mut World, hash: &str, executor: &str) -> Option<String> {
    let look = preview_of(sut, world, hash, executor);
    look["fee"]["amount"]
        .as_str()
        .and_then(vela_core::app::multisig_shell::quantity)
        .map(|fee| fee.to_string())
}

/// Execute `hash` as `executor` on the figure its sheet drew, its user
/// operation landing at once.
fn execute_landed(
    world: &mut World,
    sut: &mut Sut,
    account: &MultisigAccount,
    hash: &str,
    executor: &str,
) {
    let shown_fee = shown_fee_of(sut, world, hash, executor);
    let owners = owners_of_setup();
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.to_owned(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    run(
        sut,
        world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            executor: executor.to_owned(),
            shown_fee,
        },
    );
}

fn has_blind(review: &Value) -> bool {
    review["concerns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["kind"] == "blind")
}

/// C1 (`ProposalReview.read(blind)`): a review asked blind — a call the
/// clear reading cannot read — says sign anyway, with the Blind concern; one
/// not asked blind has neither from it.
#[test]
fn a_blind_review_says_sign_anyway_and_names_the_blind_concern() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    // The simulation reads (this world's state guard does not): a review one
    // of whose checks read is plain unless something else asks (D1).
    world.sim_reply = Some(SIM_READ.to_owned());
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let key = format!("100:{hash}");
    let review = |sut: &mut Sut, world: &mut World, blind: bool| {
        run(
            sut,
            world,
            Event::Review {
                chain_id: 100,
                safe_tx_hash: hash.clone(),
                blind,
            },
        );
        sut.view().reviews[&key].clone()
    };
    let plain = review(&mut sut, &mut world, false);
    assert!(!has_blind(&plain), "{plain}");
    assert_eq!(plain["sign"]["anyway"], false, "{plain}");
    let blind = review(&mut sut, &mut world, true);
    assert!(has_blind(&blind), "{blind}");
    assert_eq!(blind["sign"]["anyway"], true, "{blind}");
    // Another shell that sends no `blind`: not blind.
    let ev: Event =
        serde_json::from_value(json!({"type": "review", "chain_id": 100, "safe_tx_hash": hash}))
            .unwrap();
    assert!(matches!(ev, Event::Review { blind: false, .. }));
}

/// C2 (`preflight`'s one look per execution and executor): two executors
/// here, each looked at — each look under its own key; executing as one
/// sends that executor's own calls, from it.
#[test]
fn each_executor_here_has_its_own_look_and_executes_from_it() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let (a, b) = (fixture(0).address, fixture(1).address);
    let look_a = preview_of(&mut sut, &mut world, &hash, &a);
    let look_b = preview_of(&mut sut, &mut world, &hash, &b);
    let previews = sut.view().previews;
    assert_eq!(previews.len(), 2, "{previews:?}");
    assert_eq!(previews[&look_key(&hash, &a)], look_a, "never overwritten");
    assert_eq!(look_a["kind"], "priced");
    assert_eq!(look_b["kind"], "priced");
    assert_eq!(look_a["carries"], json!([b.clone()]));
    assert_eq!(look_b["carries"], json!([a.clone()]));
    assert_ne!(look_a["calls"], look_b["calls"]);
    execute_landed(&mut world, &mut sut, &account, &hash, &b);
    assert_eq!(world.sent.len(), 1);
    assert_eq!(world.sent[0].1, b, "sent from the executor picked");
    assert_eq!(
        json!(world.sent[0].2),
        look_b["calls"],
        "its own look's calls"
    );
    let kept: Value =
        serde_json::from_str(&world.store[vela_core::app::multisig_presence::EXECUTED_KEY])
            .unwrap();
    let facts = kept.as_object().unwrap().values().next().unwrap().clone();
    assert!(
        facts["approvers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x.as_str().unwrap().eq_ignore_ascii_case(&a)),
        "what its own look carried: {facts}"
    );
}

/// C2: a priced look older than PREVIEW_FRESH_MS is taken again before
/// Execute signs anything.
#[test]
fn a_stale_look_is_taken_again_before_execute() {
    use vela_core::app::multisig_proposals::PREVIEW_FRESH_MS;
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let a = fixture(0).address;
    preview_of(&mut sut, &mut world, &hash, &a);
    let looks = |w: &World| {
        w.rpc_log
            .iter()
            .filter(|(_, m, p)| {
                m == "eth_getBalance" && p[0].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&a))
            })
            .count()
    };
    let before = looks(&world);
    world.now += PREVIEW_FRESH_MS + 1.0;
    execute_landed(&mut world, &mut sut, &account, &hash, &a);
    assert!(looks(&world) > before, "looked again");
    assert_eq!(world.sent.len(), 1);
}

/// W9 (`preflight.check`): the sheet asks for a look as often as it likes;
/// the core looks again only once the last one is stale — never two
/// answers to the same question.
#[test]
fn a_fresh_look_is_not_taken_again() {
    use vela_core::app::multisig_proposals::PREVIEW_FRESH_MS;
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let a = fixture(0).address;
    let looks = |w: &World| {
        w.rpc_log
            .iter()
            .filter(|(_, m, p)| {
                m == "eth_getBalance" && p[0].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&a))
            })
            .count()
    };
    preview_of(&mut sut, &mut world, &hash, &a);
    // The world answers the look's cap even after the look won the race; a
    // shell abandons that wait — no time passes for it.
    world.now -= vela_core::app::multisig_proposals::PREVIEW_CAP_MS as f64;
    let once = looks(&world);
    assert!(once > 0);
    preview_of(&mut sut, &mut world, &hash, &a);
    assert_eq!(looks(&world), once, "a fresh look stands");
    world.now += PREVIEW_FRESH_MS + 1.0;
    preview_of(&mut sut, &mut world, &hash, &a);
    assert!(looks(&world) > once, "a stale one is taken again");
}

/// Round 5 (`PREVIEW_CAP_MS`): a look that does not answer within the cap
/// says `unchecked` — never a guess, never a button disabled for minutes.
#[test]
fn a_look_that_never_answers_says_unchecked() {
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let a = fixture(0).address;
    sut.dispatch(Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: a.clone(),
    });
    // The shell's clock runs out first: the clock and every wait are
    // answered, no read is.
    use vela_core::app::multisig_shell::MultisigOperation as Op;
    for _ in 0..50 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| matches!(op, Op::Wait { .. } | Op::Now))
            .cloned()
        else {
            break;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    let preview = sut.view().previews[&look_key(&hash, &a)].clone();
    assert_eq!(preview["kind"], "unchecked", "{preview}");
}

/// r4-5, W9: the home's "Can't run now" is the machine's — a poll looks at
/// each ready transaction by each executor here, and its standing carries
/// the look's own detail (what the multisig holds, what it needs).
#[test]
fn a_ready_one_that_would_fail_says_why_on_the_home() {
    use vela_core::app::multisig_presence::{ReadyBlock, Standing};
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
    run(&mut sut, &mut world, Event::Tick);
    let standing = sut.view().standings[&format!("100:{hash}")].clone();
    let Standing::Blocked {
        block: ReadyBlock::Fails { why },
    } = standing
    else {
        panic!("{standing:?}");
    };
    assert_eq!(why["kind"], "short", "{why}");
    // r12-1: each amount carries how it is said — Gnosis's coin, 18
    // decimals, "xDAI" — never base units a shell scales by a guess.
    assert_eq!(why["have"]["base_units"], "5");
    assert_eq!(why["need"]["base_units"], "1000");
    assert_eq!(why["need"]["said"]["kind"], "known", "{why}");
    assert_eq!(why["need"]["said"]["decimals"], 18);
    assert_eq!(why["need"]["said"]["symbol"], "xDAI");
    // r7: funded, it is noticed on a poll a sheet's pace later — not after
    // the home's minute.
    world
        .balances
        .insert((100, account.address.to_ascii_lowercase()), 5_000);
    world.now += vela_core::app::multisig_proposals::PREVIEW_FRESH_MS + 1.0;
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(
        sut.view().standings[&format!("100:{hash}")],
        Standing::Ready,
        "funded, it can run"
    );
}

/// C3: the frozen facts the timeline reads are in the view.
#[test]
fn the_frozen_facts_of_an_executed_proposal_are_in_the_view() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let a = fixture(0).address;
    execute_landed(&mut world, &mut sut, &account, &hash, &a);
    run(
        &mut sut,
        &mut world,
        Event::Timeline {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
        },
    );
    let frozen = sut.view().frozen[&format!("100:{hash}")].clone();
    assert!(frozen.executor.unwrap().eq_ignore_ascii_case(&a));
    assert_eq!(frozen.threshold, 2);
    assert_eq!(frozen.approvers.len(), 2, "{:?}", frozen.approvers);
}

/// C4 (`settleSpent`): a look that finds its number used — another member
/// executed it meanwhile — says who executed it, never "it would fail".
#[test]
fn a_look_at_a_number_used_by_another_says_it_was_executed_by_another() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let owners = owners_of_setup();
    world.deploy(
        100,
        &account.address,
        DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2),
    );
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let hash = approved_twice(&mut world, &mut sut);
    // Another member's device executes it.
    after_first(&mut world, &account);
    world.receipts.insert(
        OTHER_TX.to_owned(),
        json!({"logs": [{"address": account.address, "topics": [
            vela_core::app::multisig_proposals::EXECUTION_SUCCESS, hash
        ], "data": "0x"}]}),
    );
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "reverts", "reason": "GS025"}),
        },
    );
    let look = preview_of(&mut sut, &mut world, &hash, &fixture(0).address);
    assert_eq!(look["kind"], "spent", "{look}");
    let verdict = sut.view().executions[&format!("100:{hash}")].clone();
    assert_eq!(verdict["kind"], "executedByOther", "{verdict}");
    assert_eq!(verdict["txHash"], OTHER_TX);
    assert_eq!(verdict["paid"], false);
    assert!(world.sent.is_empty());
}

/// r9 (r8-2's class, found by the desktop lane): an address with no code
/// answers `eth_call` with "0x" — that is no answer, never 0 decimals.
#[test]
fn an_address_with_no_code_has_no_decimals() {
    const TOKEN: &str = "0x00000000000000000000000000000000000b0b0b";
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let call = &params[0];
        if method != "eth_call" || !call["to"].as_str()?.eq_ignore_ascii_case(TOKEN) {
            return None;
        }
        Some(
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!("0x"),
            },
        )
    }));
    let kind = ProposalKind::Transfers {
        transfers: vec![Transfer {
            token: Some(TOKEN.to_owned()),
            to: B.to_owned(),
            amount: "1000000".to_owned(),
        }],
    };
    let _ = propose(&mut sut, &mut world, kind, None);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let facts = sut.view().tokens[&format!("100:{TOKEN}")].clone();
    assert_eq!(
        facts.decimals, None,
        "\"0x\" is no answer: decimals unknown"
    );
}

/// r8-2: a token a proposal moves is named by its contract's own decimals,
/// never 18 assumed; a read that fails later never unsays it.
#[test]
fn a_tokens_decimals_once_read_stay_read() {
    const TOKEN: &str = "0x00000000000000000000000000000000000b0b0b";
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    let answers = std::rc::Rc::new(std::cell::Cell::new(false));
    let on = answers.clone();
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let call = &params[0];
        if method != "eth_call" || !call["to"].as_str()?.eq_ignore_ascii_case(TOKEN) {
            return None;
        }
        Some(if !on.get() {
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "the node is away".into(),
            }
        } else if call["data"] == "0x313ce567" {
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!(format!("0x{:064x}", 6)),
            }
        } else {
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!(format!("0x{:064x}{:064x}{:0<64}", 32, 3, "4f4444")),
            }
        })
    }));
    let kind = ProposalKind::Transfers {
        transfers: vec![Transfer {
            token: Some(TOKEN.to_owned()),
            to: B.to_owned(),
            amount: "10000".to_owned(),
        }],
    };
    let _ = propose(&mut sut, &mut world, kind, None);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let key = format!("100:{TOKEN}");
    let unread = sut.view().tokens[&key].clone();
    assert_eq!(unread.decimals, None, "unread is unknown, never 18");
    answers.set(true);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let read = sut.view().tokens[&key].clone();
    assert_eq!(read.decimals, Some(6));
    assert_eq!(read.symbol.as_deref(), Some("ODD"));
    answers.set(false);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert_eq!(
        sut.view().tokens[&key],
        read,
        "a failed read never unsays it"
    );
}

/// r8-17: a proposal another member executed is handed to Activity dated by
/// the block it ran in, not when this device found it.
#[test]
fn an_execution_found_later_is_dated_when_it_ran() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    after_first(&mut world, &account);
    world.receipts.insert(
        OTHER_TX.to_owned(),
        json!({"logs": [{"address": account.address, "topics": [
            vela_core::app::multisig_proposals::EXECUTION_SUCCESS, hash
        ], "data": "0x"}]}),
    );
    // It ran an hour before this device looks; the block says so.
    let ran_s = (world::T0 / 1000.0) as u64 - 3600;
    world.now = world::T0 + 600_000.0;
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        (method == "eth_getBlockByNumber" && params[0] == "0x64").then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!({"number": "0x64", "timestamp": format!("0x{ran_s:x}")}),
            }
        })
    }));
    let notices = world.notices.len();
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let row = world.notices[notices..]
        .iter()
        .find_map(|n| match n {
            Notice::ActivityRows { rows } => rows
                .iter()
                .find(|r| {
                    r["safeTxHash"]
                        .as_str()
                        .is_some_and(|h| h.eq_ignore_ascii_case(&hash))
                })
                .cloned(),
            _ => None,
        })
        .expect("the executed row is handed to Activity");
    assert_eq!(row["timestamp"].as_f64(), Some(ran_s as f64), "{row}");
}

/// C5 (r4b-7): an execution this device sent before a reload, whose user
/// operation landed, is watched again once its multisig is followed —
/// judged executed with no Execute, ended in flight, its activity row
/// handed on.
#[test]
fn an_execution_in_flight_before_a_reload_is_watched_again() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    drop(sut);
    let tx_hash = format!("0x{}", "cd".repeat(32));
    let user_op = format!("0x{}", "ab".repeat(32));
    world.store.insert(
        "vela.multisigExecuting".to_owned(),
        json!({
            format!("100:{}:{hash}", account.address.to_ascii_lowercase()): {
                "chainId": 100, "multisig": account.address, "safeTxHash": hash,
                "nonce": "0", "userOpHash": user_op, "by": fixture(0).address,
                "maybeSent": false, "startedMs": world.now, "createdMs": 1,
            }
        })
        .to_string(),
    );
    // It landed while the page was away.
    after_first(&mut world, &account);
    world.receipts.insert(
        tx_hash.clone(),
        json!({"logs": [
            {"address": account.address, "topics": [
                vela_core::app::multisig_proposals::EXECUTION_SUCCESS, hash
            ], "data": "0x"},
            {"address": "0x0000000071727De22E5E9d8BAf0edAc6f37da032", "topics": [
                "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f",
                user_op
            ], "data": "0x"}
        ]}),
    );
    world.user_op = Some(("included".to_owned(), Some(tx_hash.clone())));
    let notices = world.notices.len();
    let mut next = Sut::new();
    let ev = start(&world, vec![100, 8453]);
    run(&mut next, &mut world, ev);
    run(
        &mut next,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    let verdict = next.view().executions[&format!("100:{hash}")].clone();
    assert_eq!(verdict["kind"], "executed", "{verdict}");
    assert_eq!(verdict["txHash"], tx_hash);
    assert!(world.sent.is_empty(), "no Execute");
    assert!(
        !world.store.contains_key("vela.multisigExecuting"),
        "no longer in flight"
    );
    assert!(world.notices[notices..].iter().any(|n| matches!(
        n,
        Notice::ActivityRows { rows } if rows.iter().any(|r| r["userOpHash"] == user_op)
    )));
}

/// C6 (`lastActor.note`): the member used here last, for approving and
/// executing too, when more than one member is here; none noted with one.
#[test]
fn the_member_used_last_is_noted_for_approve_and_execute() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let b = fixture(1).address.to_ascii_lowercase();
    assert_eq!(sut.view().last_actor.as_deref(), Some(b.as_str()));
    let kept: Value = serde_json::from_str(&world.store["vela.multisigLastActor"]).unwrap();
    assert_eq!(kept[account.address.to_ascii_lowercase()], b);
    let a = fixture(0).address;
    execute_landed(&mut world, &mut sut, &account, &hash, &a);
    assert_eq!(
        sut.view().last_actor,
        Some(a.to_ascii_lowercase()),
        "the executor"
    );

    let (mut world, mut sut, _) = setup(&[0]);
    hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    assert_eq!(
        sut.view().last_actor,
        None,
        "one member here: nothing to pick"
    );
}

/// C7: a link's proposal on a multisig with no member here is reviewed by
/// `Review` too (blind, when the sheet asks so).
#[test]
fn a_linked_previews_proposal_is_reviewed() {
    const OUTSIDE: &str = "0x00000000000000000000000000000000000000a7";
    let g = genesis(&[OUTSIDE], 1, "0x5c");
    let account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(stored(&account, 100, &tx, vec![]));
    let mut sut = Sut::new();
    let ev = start(&world, vec![100]);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            preview: Some(Box::new(account.clone())),
        },
    );
    assert_eq!(sut.view().followed, None);
    run(
        &mut sut,
        &mut world,
        Event::Review {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            blind: true,
        },
    );
    let review = sut.view().reviews[&format!("100:{hash}")].clone();
    assert!(review.is_object(), "{review}");
    assert!(
        has_blind(&review),
        "this Review's, not the link's own: {review}"
    );
}

/// W9 (`membersAnywhere`): the followed multisig's members anywhere — its
/// original members while it is deployed nowhere, then the members a network
/// it is deployed on has now (a member added there counts).
#[test]
fn the_members_anywhere_are_the_machine_s() {
    let (mut world, mut sut, account) = setup(&[0]);
    let has = |v: &[String], who: &str| v.iter().any(|x| x.eq_ignore_ascii_case(who));
    let before = sut.view().members_anywhere;
    assert_eq!(before.len(), 3, "{before:?}");
    let mut owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    owners.push(B.to_owned());
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    world.deploy(100, &account.address, DeployedSafe::clean(&refs, 2));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let after = sut.view().members_anywhere;
    assert!(has(&after, B), "{after:?}");
    assert_eq!(after.len(), 4);
}

/// W9 (onchain run 2): a look that said the executor cannot pay is never what
/// Execute stops on — funded since, it is looked at again and executes.
#[test]
fn an_executor_funded_after_its_look_executes() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    let a = fixture(0).address;
    world.balances.insert((100, a.to_ascii_lowercase()), 0);
    let look = preview_of(&mut sut, &mut world, &hash, &a);
    assert_eq!(look["canPay"], false, "{look}");
    // The world answers the look's cap after the look; no time passes for it.
    world.now -= vela_core::app::multisig_proposals::PREVIEW_CAP_MS as f64;
    world
        .balances
        .insert((100, a.to_ascii_lowercase()), 10_000_000_000_000_000_000);
    execute_landed(&mut world, &mut sut, &account, &hash, &a);
    assert_eq!(world.sent.len(), 1, "{:?}", sut.view().actions);
}

// -- core-ms-2: "Approve and execute", one press ---------------------------------------------

/// The member whose approval reaches the threshold, whose look says it can
/// pay, is offered the one press — and it signs ONE operation, its approval
/// counted as the executor's; the others approve only.
#[test]
fn approve_and_execute_is_offered_and_runs_in_one_press() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let key = format!("100:{}", hash.to_ascii_lowercase());
    assert!(
        !sut.view().approve_execute.contains_key(&key),
        "not before a look says it can pay"
    );
    let b = fixture(1).address;
    let preview = preview_of(&mut sut, &mut world, &hash, &b);
    assert_eq!(preview["kind"], "priced", "{preview}");
    assert_eq!(sut.view().approve_execute[&key], vec![b.clone()]);
    let passkeys = world.passkeys.len();
    approve_execute_landed(&mut world, &mut sut, &account, &hash, &b);
    assert_eq!(world.sent.len(), 1, "one user operation");
    assert_eq!(world.passkeys.len(), passkeys, "no separate approval asked");
    assert!(matches!(
        sut.view().actions[&format!("execute:{}", hash.to_ascii_lowercase())],
        ActionState::Done { .. }
    ));
    assert!(!sut
        .view()
        .actions
        .contains_key(&format!("approve:{}", hash.to_ascii_lowercase())));
    let verdict = sut.view().executions[&key].clone();
    assert_eq!(verdict["kind"], "executed", "{verdict}");
    assert!(sut.view().approve_execute.is_empty());
}

/// r16-4 (supersedes r4-11): the one press does both or nothing. Stopped
/// before any signature (here: it would fail), nothing is approved either —
/// the execution's stop is said beside the press, the approval is the
/// person's own next press.
#[test]
fn approve_and_execute_stopped_before_signing_approves_nothing() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1000"), None));
    world
        .balances
        .insert((100, account.address.to_ascii_lowercase()), 5);
    let b = fixture(1).address;
    // A look that says it would fail offers no one press.
    assert_eq!(preview_of(&mut sut, &mut world, &hash, &b)["kind"], "fails");
    assert!(sut.view().approve_execute.is_empty());
    let passkeys = world.passkeys.len();
    run(
        &mut sut,
        &mut world,
        Event::ApproveExecute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: b,
            shown_fee: None,
        },
    );
    let lower = hash.to_ascii_lowercase();
    let ActionState::Stopped { preview } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    assert_eq!(preview["kind"], "fails");
    assert!(
        !sut.view().actions.contains_key(&format!("approve:{lower}")),
        "{:?}",
        sut.view().actions
    );
    assert!(world.sent.is_empty());
    assert_eq!(world.passkeys.len(), passkeys, "nothing signed");
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.approved, 1, "no approval behind the person's back");
}

/// r16-4 (Android, Arc rate-limited): the sheet drew a priced fee, the look
/// aged, and at the press its re-quote could not price it. The one press
/// approved alone and said nothing (the stop was "quiet", r11-9), and the
/// sheet then read "2/2" over a plain Execute. Now: nothing is signed, the
/// stop says the fee could not be worked out (`stopped {unpriced}`, beside
/// the press, with Check again); a later look that prices it lifts the stop
/// and the one press is offered on the new figure.
#[test]
fn a_one_press_whose_fee_is_unknown_at_the_press_signs_nothing_and_says_why() {
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let lower = hash.to_ascii_lowercase();
    let b = fixture(1).address;
    let look = preview_of(&mut sut, &mut world, &hash, &b);
    assert_eq!(look["kind"], "priced", "{look}");
    let shown = vela_core::app::multisig_shell::quantity(look["fee"]["amount"].as_str().unwrap())
        .unwrap()
        .to_string();
    // The press comes after the look's window, and the network will not price it.
    world.now += 20_000.0;
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "failed"}),
        },
    );
    let passkeys = world.passkeys.len();
    run(
        &mut sut,
        &mut world,
        Event::ApproveExecute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: b.clone(),
            shown_fee: Some(shown),
        },
    );
    let ActionState::Stopped { preview } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("said beside the press: {:?}", sut.view().actions);
    };
    assert_eq!(preview["kind"], "unpriced");
    assert!(
        !sut.view().actions.contains_key(&format!("approve:{lower}")),
        "{:?}",
        sut.view().actions
    );
    assert_eq!(world.passkeys.len(), passkeys, "nothing signed");
    assert!(world.sent.is_empty());
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.approved, 1);
    // Check again: the network prices it now — the stop is lifted, the new
    // figure is drawn, and the one press is offered on it.
    world.quote_answer = None;
    world.now += 20_000.0;
    let again = preview_of(&mut sut, &mut world, &hash, &b);
    assert_eq!(again["kind"], "priced", "{again}");
    assert!(
        !sut.view().actions.contains_key(&format!("execute:{lower}")),
        "a stop for a fee not worked out goes once it is: {:?}",
        sut.view().actions
    );
    assert_eq!(sut.view().approve_execute[&format!("100:{lower}")], vec![b]);
}

/// r16-4: a one press over a look that priced nothing (the sheet drew no
/// figure) never signs a fee the person did not see: a re-quote that prices
/// it at the press stops with `fee_moved` (the figure said), nothing signed;
/// the next press on that figure does both.
#[test]
fn a_one_press_over_no_figure_signs_nothing_until_a_figure_is_shown() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let lower = hash.to_ascii_lowercase();
    let b = fixture(1).address;
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "failed"}),
        },
    );
    assert_eq!(
        preview_of(&mut sut, &mut world, &hash, &b)["kind"],
        "unpriced"
    );
    assert_eq!(
        sut.view().approve_execute[&format!("100:{lower}")],
        vec![b.clone()]
    );
    world.quote_answer = None;
    world.now += 20_000.0;
    let passkeys = world.passkeys.len();
    let press = |shown: Option<String>| Event::ApproveExecute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: b.clone(),
        shown_fee: shown,
    };
    run(&mut sut, &mut world, press(None));
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    let detail: Value = serde_json::from_str(&detail).expect("a failure the shells read");
    assert_eq!(detail["kind"], "fee_moved");
    assert!(!sut.view().actions.contains_key(&format!("approve:{lower}")));
    assert_eq!(world.passkeys.len(), passkeys, "nothing signed");
    assert!(world.sent.is_empty());
    // Pressed on the figure now shown: both, in one operation.
    let owners = owners_of_setup();
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
    run(
        &mut sut,
        &mut world,
        press(detail["amount"].as_str().map(str::to_owned)),
    );
    assert_eq!(world.sent.len(), 1, "signed at the figure shown");
    assert!(matches!(
        sut.view().actions[&format!("execute:{lower}")],
        ActionState::Done { .. }
    ));
}

/// Round 16 leftover 1 (§26 left open): a plain Execute over a look that
/// priced nothing (the sheet drew no figure) never signs a figure priced only
/// at the press — the one press's rule (`execute_as`), now every press's:
/// `fee_moved` with that figure, nothing signed; the next press on the figure
/// now shown executes.
#[test]
fn a_plain_execute_over_no_figure_signs_nothing_until_a_figure_is_shown() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let lower = hash.to_ascii_lowercase();
    let (a, b) = (fixture(0).address, fixture(1).address);
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: b.clone(),
        },
    );
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "failed"}),
        },
    );
    assert_eq!(
        preview_of(&mut sut, &mut world, &hash, &a)["kind"],
        "unpriced"
    );
    world.quote_answer = None;
    world.now += 20_000.0;
    let passkeys = world.passkeys.len();
    let press = |shown: Option<String>| Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: a.clone(),
        shown_fee: shown,
    };
    run(&mut sut, &mut world, press(None));
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    let detail: Value = serde_json::from_str(&detail).expect("a failure the shells read");
    assert_eq!(detail["kind"], "fee_moved");
    assert_eq!(world.passkeys.len(), passkeys, "nothing signed");
    assert!(world.sent.is_empty(), "nothing sent");
    // Pressed on the figure now shown: executed.
    let owners = owners_of_setup();
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
    run(
        &mut sut,
        &mut world,
        press(detail["amount"].as_str().map(str::to_owned)),
    );
    assert_eq!(world.sent.len(), 1, "signed at the figure shown");
    assert!(matches!(
        sut.view().actions[&format!("execute:{lower}")],
        ActionState::Done { .. }
    ));
}

/// Round 16 leftover 4: a failure is a stable kind the shells word from the
/// catalog — never English diagnostics. The core's own refusals carry a kind
/// (`unknown` with a stable `reason`), and a shell's send failure written as
/// plain text is served as `unknown` with no text at all.
#[test]
fn failures_are_stable_kinds_never_english_text() {
    use vela_core::app::multisig_failure::{failure_said, FailureKind};
    let (mut world, mut sut, _account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let lower = hash.to_ascii_lowercase();
    let a = fixture(0).address;
    // An executor that cannot execute it now (one approval of two).
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: a.clone(),
            shown_fee: None,
        },
    );
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    let said: Value = serde_json::from_str(&detail).expect("JSON, never English");
    assert_eq!(said["kind"], "unknown", "{detail}");
    assert!(said["reason"].is_string(), "{detail}");
    assert_eq!(failure_said(&detail).kind, FailureKind::Unknown);
    assert_eq!(
        failure_said(&detail).key.as_deref(),
        Some("home.multisig.proposal.reason.unknown")
    );
    // A shell's send failure written as plain text is served without it.
    let b = fixture(1).address;
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: b,
        },
    );
    let look = preview_of(&mut sut, &mut world, &hash, &a);
    assert_eq!(look["kind"], "priced", "{look}");
    world.send_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::CallsFailed {
            detail: "Error: relay said 500 Internal Server Error".to_owned(),
            maybe_sent: false,
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: a.clone(),
            shown_fee: Some(
                vela_core::app::multisig_shell::quantity(look["fee"]["amount"].as_str().unwrap())
                    .unwrap()
                    .to_string(),
            ),
        },
    );
    let ActionState::Failed { detail } = sut.view().actions[&format!("execute:{lower}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    assert!(!detail.contains("relay said"), "{detail}");
    let said: Value = serde_json::from_str(&detail).expect("JSON, never English");
    assert_eq!(said["kind"], "unknown", "{detail}");
    // Every shell's words for a detail come from its kind.
    for (detail, kind, key) in [
        (
            r#"{"kind":"passkey_check"}"#,
            FailureKind::PasskeyCheck,
            Some("home.multisig.proposal.reason.passkeyFailed"),
        ),
        (
            "the passkey's signature did not check out: bad s",
            FailureKind::Unknown,
            Some("home.multisig.proposal.reason.unknown"),
        ),
        (
            r#"{"kind":"not_sent"}"#,
            FailureKind::NotSent,
            Some("home.multisig.proposal.notSent"),
        ),
        (
            r#"{"kind":"fee_moved","amount":"1","chainId":100,"executor":"0xa"}"#,
            FailureKind::FeeMoved,
            None,
        ),
        (
            r#"{"kind":"unknown","message":"TypeError: x is undefined"}"#,
            FailureKind::Unknown,
            Some("home.multisig.proposal.reason.unknown"),
        ),
    ] {
        let said = failure_said(detail);
        assert_eq!(said.kind, kind, "{detail}");
        assert_eq!(said.key.as_deref(), key, "{detail}");
    }
}

/// [`execute_landed`] through `ApproveExecute`.
fn approve_execute_landed(
    world: &mut World,
    sut: &mut Sut,
    account: &MultisigAccount,
    hash: &str,
    executor: &str,
) {
    let owners = owners_of_setup();
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.to_owned(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    run(
        sut,
        world,
        Event::ApproveExecute {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            executor: executor.to_owned(),
            shown_fee: None,
        },
    );
}

// -- core-ms-2: the header chip and the home's line -------------------------------------------

/// The rule chip is the rule in force where it is used (the original before
/// it is deployed, its members now after); the home's line says what waits,
/// in order, and opens the first; `set_up` once it is deployed somewhere.
#[test]
fn the_rule_chip_and_the_waiting_line_are_the_machines() {
    use vela_core::app::multisig_members::{MemberRule, RuleChip};
    use vela_core::app::multisig_presence::{WaitingKind, WaitingPart};
    let (mut world, mut sut, account) = setup(&[0, 1]);
    assert_eq!(
        sut.view().rule_chip,
        Some(RuleChip::Needs {
            rule: MemberRule::of(2, 3)
        })
    );
    assert!(!sut.view().set_up);
    assert_eq!(sut.view().banner, None, "nothing waits");
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let banner = sut.view().banner.clone().unwrap();
    assert_eq!(
        banner.parts,
        vec![WaitingPart {
            kind: WaitingKind::NeedsYou,
            count: 1
        }]
    );
    assert_eq!(
        banner.first.safe_tx_hash.to_ascii_lowercase(),
        hash.to_ascii_lowercase()
    );
    assert_eq!(
        sut.view().waiting_parts[&account.address.to_ascii_lowercase()],
        banner.parts
    );
    // Deployed with another member and rule since: its members now.
    let mut owners = owners_of_setup();
    owners.push("0x00000000000000000000000000000000000000ee".to_owned());
    world.deploy(
        100,
        &account.address,
        DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 3),
    );
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert!(sut.view().set_up);
    assert_eq!(
        sut.view().rule_chip,
        Some(RuleChip::Needs {
            rule: MemberRule::of(3, 4)
        })
    );
}

// -- r10-1: whose outage it is ------------------------------------------------------------

/// r10-1: the chain is dead and the service answered — "Can't reach <network>",
/// never "The multisig service can't be reached".
#[test]
fn a_dead_chain_is_never_said_as_the_service() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.silent.insert(100);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let view = sut.view();
    assert_eq!(view.unreadable, vec![100]);
    assert!(!view.unreachable, "the service answered");
}

/// r10-1: the service is away AND the chain this device reads it on is dead —
/// the service reads that chain too, so its silence there is the chain's:
/// the network is named, not the service.
#[test]
fn a_service_silent_on_a_dead_chain_is_that_chain() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.silent.insert(100);
    world.service_down = true;
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let view = sut.view();
    assert_eq!(view.unreadable, vec![100]);
    assert!(!view.unreachable);
}

/// The service really away, the chain fine: said as the service.
#[test]
fn a_service_away_on_a_live_chain_is_the_service() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.service_down = true;
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let view = sut.view();
    assert!(view.unreadable.is_empty());
    assert!(view.unreachable, "the service's own read missed");
}

/// r9-14: the rule as the chain says (a member added: 2 of 4) is told to the
/// accounts machine with the members, so its row says it unfollowed too.
#[test]
fn the_rule_the_chain_says_is_told() {
    use vela_core::app::multisig_members::{MemberRule, RuleChip};
    let (mut world, mut sut, account) = setup(&[0]);
    let mut owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    owners.push(B.to_owned());
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    world.deploy(100, &account.address, world::DeployedSafe::clean(&refs, 2));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert!(world.notices.contains(&Notice::Account {
        address: account.address.clone(),
        update: AccountUpdate::RuleNow {
            chip: RuleChip::Needs {
                rule: MemberRule::of(2, 4)
            }
        },
    }));
}

/// r12 S4: one note for one fact — a proposal whose look says it would fail
/// is not also "expected to fail" by its review: the reverts concern goes,
/// a reverting simulation is served withheld. Another proposal is untouched.
#[test]
fn a_failure_the_look_says_is_not_said_again_by_the_review() {
    use std::collections::BTreeMap;
    use vela_core::app::multisig_proposals::{one_note_per_failure, ActionState};
    let review = serde_json::json!({
        "concerns": [{"kind": "reverts"}, {"kind": "firstTime", "address": "0x01"}],
        "simulation": {"kind": "reverts", "deltas": [], "revert_reason": null, "notice_risk": "danger",
            "notice_key": "componentsUi.signing.simWillFail", "no_change_key": null, "recheck": false},
        "movesTitleKey": "componentsUi.signing.balanceChangesTitle"
    });
    let reviews = BTreeMap::from([
        ("100:0xAB".to_owned(), review.clone()),
        ("100:0xcd".to_owned(), review.clone()),
    ]);
    let previews = BTreeMap::from([(
        "100:0xab:0xexecutor".to_owned(),
        serde_json::json!({"kind": "fails", "why": {"kind": "reverts", "reason": null}}),
    )]);
    let out = one_note_per_failure(&reviews, &previews, &BTreeMap::<String, ActionState>::new());
    let failing = &out["100:0xAB"];
    assert_eq!(
        failing["concerns"],
        serde_json::json!([{"kind": "firstTime", "address": "0x01"}])
    );
    assert_eq!(failing["simulation"]["kind"], "withheld");
    assert!(failing["simulation"]["notice_key"].is_null());
    assert!(failing["movesTitleKey"].is_null());
    assert_eq!(
        out["100:0xcd"], review,
        "a proposal no look says fails keeps its review"
    );
}

/// Round 17 (r17-2): after a plain Execute over no figure (§27 item 1) the
/// stop "Network fee ≈ 0.3022" stayed for 20 s on the web while the fee
/// line beside it re-quoted to 0.2325, and the next press charged 0.2325
/// with no new stop. The §25 rule covers this path too: the stop the press
/// over no figure said is its executor's look, and it follows every
/// re-quote of that look — the figure the next press signs. (The core
/// served it so; the web drew the words it had kept at the press.)
#[test]
fn a_no_figure_stop_follows_the_requote_and_the_next_press_signs_it() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    let a = fixture(0).address;
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    let quote = |amount: &str| vela_core::app::multisig_shell::MultisigShellResult::Quoted {
        quote: json!({"kind": "quoted", "fee": {"token": null, "amount": amount}, "totalGas": "0x1", "executorDeployed": true}),
    };
    world.quote_answer = Some(
        vela_core::app::multisig_shell::MultisigShellResult::Quoted {
            quote: json!({"kind": "failed"}),
        },
    );
    assert_eq!(
        preview_of(&mut sut, &mut world, &hash, &a)["kind"],
        "unpriced"
    );
    // Pressed over no figure: the figure priced at the press is said.
    world.now += 20_000.0;
    world.quote_answer = Some(quote("0x300"));
    let press = |shown: Option<&str>| Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: a.clone(),
        shown_fee: shown.map(str::to_owned),
    };
    run(&mut sut, &mut world, press(None));
    let key = format!("execute:{}", hash.to_ascii_lowercase());
    let stop = |sut: &Sut| -> Value {
        match sut.view().actions.get(&key) {
            Some(ActionState::Failed { detail }) => serde_json::from_str(detail).expect("json"),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(stop(&sut)["kind"], "fee_moved");
    assert_eq!(stop(&sut)["amount"], "768");
    assert!(world.sent.is_empty(), "nothing signed");
    // The fee line re-quotes beside the stop: the stop follows it.
    world.now += 20_000.0;
    world.quote_answer = Some(quote("0x200"));
    run(
        &mut sut,
        &mut world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: a.clone(),
        },
    );
    assert_eq!(
        sut.view().previews[&look_key(&hash, &a)]["fee"]["amount"],
        json!("0x200"),
        "the fee line's figure"
    );
    assert_eq!(
        stop(&sut)["amount"],
        "512",
        "the stop names the fee line's figure"
    );
    // The next press, on that figure, signs exactly it.
    run(&mut sut, &mut world, press(Some("512")));
    assert_eq!(world.sent.len(), 1, "signed at the figure the stop named");
}
