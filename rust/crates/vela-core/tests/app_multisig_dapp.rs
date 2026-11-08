//! A dApp's transaction from a multisig, held by the proposals machine —
//! the web's `dapp-request.test.ts` and the multisig part of
//! `DappRequestHost` (`followMultisig`, `answerBatch`, `tellProposed`): its
//! review comes first and nothing is signed before "Propose and approve";
//! every way out answers the site once and honestly; an
//! `eth_sendTransaction` is held until its proposal executes and answered
//! with that transaction; a `wallet_sendCalls` is one proposal of every call,
//! answered its id, followed by `wallet_getCallsStatus`.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_dapp::{
    left_proposed, multisig_refusal, DappAnswer, HeldStage, ANSWER_MARGIN_MS, PROPOSED_CODE,
};
use vela_core::app::multisig_proposals::{ActionState, Event, MultisigProposals, ProposalStatus};
use vela_core::app::multisig_shell::{MultisigAccount, MultisigOperation as Op};
use vela_core::multisig;
use world::{fixture, genesis, settle, settle_reads_first, DeployedSafe, Landing, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

const DEAD: &str = "0x000000000000000000000000000000000000dEaD";
const BEEF: &str = "0x000000000000000000000000000000000000bEEF";
const LIMIT_MS: f64 = 300_000.0;

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    // A shell's clock: a bound (the review's) runs out after the reads it bounds.
    let answered = settle_reads_first(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// As [`run`], leaving long waits (the request's limit) outstanding.
fn run_short(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    for _ in 0..20_000 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| {
                outstanding
                    .iter()
                    .find(|op| !matches!(op, Op::Wait { ms } if *ms >= 100_000))
            })
            .cloned()
        else {
            return;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    panic!("the machine never settled");
}

fn long_waits(sut: &Sut) -> Vec<u32> {
    sut.outstanding()
        .iter()
        .filter_map(|op| match op {
            Op::Wait { ms } if *ms >= 100_000 => Some(*ms),
            _ => None,
        })
        .collect()
}

/// A 2-of-3 of the fixtures used on 100; `here`: which are on this device.
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
    let ev = Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(&mut sut, &mut world, ev);
    (world, sut, account)
}

fn ask(account: &MultisigAccount, id: &str, method: &str, params: Value) -> Event {
    Event::DappRequest {
        id: id.to_owned(),
        account: Box::new(account.clone()),
        method: method.to_owned(),
        params,
        origin: "https://app.example".to_owned(),
        chain_id: 100,
        limit_ms: Some(LIMIT_MS),
    }
}

fn send_tx(account: &MultisigAccount) -> Event {
    ask(
        account,
        "r1",
        "eth_sendTransaction",
        json!([{ "from": account.address, "to": DEAD, "value": "0x1", "data": "0x" }]),
    )
}

fn answer(sut: &Sut, id: &str) -> Option<DappAnswer> {
    sut.view().dapp[id].answer.clone()
}

/// Executes the proposal `hash` (approved by 0 and 1) from this device:
/// the chain then holds its `ExecutionSuccess` in `tx_hash`.
fn execute(sut: &mut Sut, world: &mut World, account: &MultisigAccount, hash: &str) -> String {
    run_short(
        sut,
        world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            owner: fixture(1).address,
        },
    );
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    let tx_hash = format!("0x{}", "cd".repeat(32));
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.to_owned(),
        tx_hash: tx_hash.clone(),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    // The sheet's look first, its figure carried on the press (§24; round
    // 16 leftover 1: a press over no figure signs nothing).
    run_short(
        sut,
        world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            executor: fixture(0).address,
        },
    );
    let key = format!(
        "100:{}:{}",
        hash.to_ascii_lowercase(),
        fixture(0).address.to_ascii_lowercase()
    );
    let shown_fee = sut.view().previews.get(&key).and_then(|look| {
        look["fee"]["amount"]
            .as_str()
            .and_then(vela_core::app::multisig_shell::quantity)
            .map(|fee| fee.to_string())
    });
    run_short(
        sut,
        world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            executor: fixture(0).address,
            shown_fee,
        },
    );
    tx_hash
}

/// dapp-1: the review first, nothing signed until "Propose and approve";
/// proposed is not answered — the site waits for the execution.
#[test]
fn its_review_comes_first_and_proposed_is_not_answered() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    run(&mut sut, &mut world, send_tx(&account));
    let view = sut.view();
    assert_eq!(view.followed.as_deref(), Some(account.address.as_str()));
    assert_eq!(view.dapp["r1"].stage, HeldStage::Reviewing);
    assert!(view.draft.is_some(), "the draft, with its review");
    assert!(world.passkeys.is_empty(), "nothing signed");
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(world.passkeys.len(), 1, "one passkey prompt");
    let held = sut.view().dapp["r1"].clone();
    assert_eq!(held.stage, HeldStage::Waiting);
    assert!(held.safe_tx_hash.is_some());
    assert_eq!(held.answer, None, "held until it executes");
    assert_eq!(
        long_waits(&sut),
        vec![(LIMIT_MS - ANSWER_MARGIN_MS) as u32],
        "told before the limit, counted from the claim before the passkey"
    );
}

/// Decline, or a close before proposing, is the person rejecting it (4001).
#[test]
fn a_close_before_proposing_is_4001_and_signs_nothing() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, send_tx(&account));
    run(&mut sut, &mut world, Event::DappLeave { id: "r1".into() });
    let a = answer(&sut, "r1").unwrap();
    assert_eq!(a.error.unwrap().code, 4001);
    assert!(sut.view().draft.is_none(), "the draft is withdrawn");
    assert!(world.passkeys.is_empty());
    // The person's own Withdraw is the same decline.
    run(&mut sut, &mut world, send_tx(&account));
    run(&mut sut, &mut world, Event::Withdraw);
    assert_eq!(answer(&sut, "r1").unwrap().error.unwrap().code, 4001);
}

/// A close after proposing says it was proposed — never a rejection, never
/// a blank wait; answered once.
#[test]
fn a_close_after_proposing_says_proposed_once() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, send_tx(&account));
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let hash = sut.view().dapp["r1"].safe_tx_hash.clone().unwrap();
    run_short(&mut sut, &mut world, Event::DappLeave { id: "r1".into() });
    let error = answer(&sut, "r1").unwrap().error.unwrap();
    assert_eq!(error, left_proposed(&hash));
    assert_eq!(error.code, PROPOSED_CODE);
    assert!(error.message.contains(&hash));
    assert_eq!(
        error.data,
        Some(json!({ "safeTxHash": hash, "status": "proposed" }))
    );
    // The limit runs out later: the answer stays the one given.
    settle(&mut sut, &mut world, 1_000);
    assert_eq!(answer(&sut, "r1").unwrap().error.unwrap(), error);
    assert!(!sut.view().dapp["r1"].told);
    run(&mut sut, &mut world, Event::DappSettled { id: "r1".into() });
    assert!(sut.view().dapp.is_empty());
}

/// Waiting for approvals: the site is told it was proposed before the limit.
#[test]
fn the_site_is_told_proposed_before_the_limit() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, send_tx(&account));
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(answer(&sut, "r1"), None);
    let t0 = world.now;
    settle(&mut sut, &mut world, 1_000);
    let held = sut.view().dapp["r1"].clone();
    assert!(held.told);
    let hash = held.safe_tx_hash.unwrap();
    assert_eq!(held.answer.unwrap().error.unwrap(), left_proposed(&hash));
    assert_eq!(world.now - t0, LIMIT_MS - ANSWER_MARGIN_MS);
}

/// Executed: the site gets the real transaction hash, and no "proposed" later.
#[test]
fn executed_the_site_gets_the_transaction_that_ran_it() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    run(&mut sut, &mut world, send_tx(&account));
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let hash = sut.view().dapp["r1"].safe_tx_hash.clone().unwrap();
    let tx = execute(&mut sut, &mut world, &account, &hash);
    assert_eq!(answer(&sut, "r1").unwrap().result, Some(json!(tx)));
    settle(&mut sut, &mut world, 1_000);
    let held = sut.view().dapp["r1"].clone();
    assert!(!held.told);
    assert_eq!(held.answer.unwrap().result, Some(json!(tx)));
}

/// Done at its number but not by this transaction: the members cancelled it.
#[test]
fn its_number_used_by_another_is_the_members_cancelling_it() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, send_tx(&account));
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut safe = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    safe.nonce = 1;
    world.deploy(100, &account.address, safe);
    run_short(&mut sut, &mut world, Event::Refresh { fresh: true });
    let item = sut.view().items[0].clone();
    assert_eq!(item.status, ProposalStatus::Done);
    let error = answer(&sut, "r1").unwrap().error.unwrap();
    assert_eq!(error.code, 4001);
    assert!(error.message.contains("cancelled"));
}

/// A dismissed passkey leaves the review up, with nothing answered.
#[test]
fn a_dismissed_passkey_answers_nothing() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, send_tx(&account));
    world.cancel_passkey = true;
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let view = sut.view();
    assert_eq!(view.actions["propose"], ActionState::Cancelled);
    assert_eq!(view.dapp["r1"].stage, HeldStage::Reviewing);
    assert_eq!(view.dapp["r1"].answer, None);
    assert!(view.draft.is_some());
}

/// dapp-2: a batch is one proposal of every call, its id answered once
/// proposed; `wallet_getCallsStatus` says pending, then the receipt.
#[test]
fn a_batch_is_one_proposal_answered_its_id_then_followed() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let calls = vec![
        multisig::CallArg {
            to: DEAD.into(),
            value: "0x1".into(),
            data: String::new(),
        },
        multisig::CallArg {
            to: BEEF.into(),
            value: "0x0".into(),
            data: "0x".into(),
        },
    ];
    let multi_send = multisig::calls_tx(&calls, "0x0").unwrap().to;
    world
        .codes
        .insert((100, multi_send.to_ascii_lowercase()), "0x6080".into());
    run(
        &mut sut,
        &mut world,
        ask(
            &account,
            "b1",
            "wallet_sendCalls",
            json!([{ "version": "2.0.0", "chainId": "0x64", "from": account.address,
                "atomicRequired": true,
                "calls": [{ "to": DEAD, "value": "0x1" }, { "to": BEEF, "value": "0x0", "data": "0x" }] }]),
        ),
    );
    let held = sut.view().dapp["b1"].clone();
    assert_eq!(held.calls, calls, "every call, in the one proposal");
    assert!(world.passkeys.is_empty());
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let held = sut.view().dapp["b1"].clone();
    let hash = held.safe_tx_hash.clone().unwrap();
    // Where the shell tells its site's status reader to look — the
    // proposal's number — is known with the answer, whatever the list has
    // read back yet.
    assert_eq!(held.nonce.as_deref(), Some("0"), "{held:?}");
    assert_eq!(held.answer.unwrap().result, Some(json!({ "id": hash })));
    assert!(long_waits(&sut).is_empty(), "never followed for a hash");

    let status = |sut: &mut Sut, world: &mut World| {
        run_short(
            sut,
            world,
            Event::CallsStatus {
                id: "s".into(),
                chain_id: 100,
                calls_id: hash.clone(),
            },
        );
        sut.view().calls_status["s"].result.clone().unwrap()
    };
    assert_eq!(status(&mut sut, &mut world)["status"], 100, "pending");
    let tx = execute(&mut sut, &mut world, &account, &hash);
    let receipt = world.receipts.get_mut(&tx).unwrap();
    receipt["transactionHash"] = json!(tx);
    receipt["status"] = json!("0x1");
    receipt["blockNumber"] = json!("0x64");
    let done = status(&mut sut, &mut world);
    assert_eq!(done["status"], 200);
    assert_eq!(done["receipts"][0]["transactionHash"], json!(tx));
    // An id this multisig never answered with is unknown.
    run_short(
        &mut sut,
        &mut world,
        Event::CallsStatus {
            id: "u".into(),
            chain_id: 100,
            calls_id: format!("0x{}", "12".repeat(32)),
        },
    );
    assert_eq!(
        sut.view().calls_status["u"].error.clone().unwrap().code,
        5730
    );
}

/// A batch proposed while a read of the list is already on its way (the
/// shells' poll, every few seconds while the sheet is open): the read that
/// would bring it back is queued behind that one, and the site is answered
/// first. Its number is the one it was drafted and signed at — what the
/// shell hands its status reader (`wallet_getCallsStatus` reads the
/// multisig's nonce against it); without it the batch is never followed.
#[test]
fn a_batch_proposed_while_the_list_is_read_still_says_its_number() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    run(
        &mut sut,
        &mut world,
        ask(
            &account,
            "b1",
            "wallet_sendCalls",
            json!([{ "version": "2.0.0", "chainId": "0x64", "from": account.address,
                "atomicRequired": true, "calls": [{ "to": DEAD, "value": "0x1" }] }]),
        ),
    );
    assert!(sut.view().draft.is_some(), "its review first");
    // A poll's read of the list, on its way to the service and not back yet.
    let before = sut.outstanding();
    sut.dispatch(Event::Refresh { fresh: false });
    let mut held_back = Vec::new();
    for _ in 0..100 {
        let fresh: Vec<Op> = sut
            .outstanding()
            .into_iter()
            .filter(|op| !before.contains(op))
            .collect();
        if let Some(read) = fresh.iter().find(|op| matches!(op, Op::Http { .. })) {
            held_back.push(read.clone());
            break;
        }
        let Some(op) = fresh.first().cloned() else {
            break;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    assert_eq!(held_back.len(), 1, "the poll's read is out");
    sut.dispatch(Event::ConfirmDraft { as_member: None });
    for _ in 0..20_000 {
        let next = sut
            .outstanding()
            .into_iter()
            .find(|op| !held_back.contains(op) && !matches!(op, Op::Wait { ms } if *ms >= 100_000));
        let Some(op) = next else { break };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
        if sut.view().dapp["b1"].answer.is_some() {
            break;
        }
    }
    let held = sut.view().dapp["b1"].clone();
    let hash = held.safe_tx_hash.clone().expect("proposed");
    assert_eq!(held.answer.unwrap().result, Some(json!({ "id": hash })));
    assert_eq!(held.nonce.as_deref(), Some("0"), "{:?}", held.nonce);
}

/// dapp-7: a multisig signs no messages yet — said in the window first, the
/// site answered once the person has read it.
#[test]
fn a_message_is_refused_once_read() {
    assert!(multisig_refusal("eth_sendTransaction").is_none());
    assert!(multisig_refusal("wallet_sendCalls").is_none());
    assert_eq!(multisig_refusal("eth_signTypedData_v4").unwrap().code, 4200);
    let (mut world, mut sut, account) = setup(&[0]);
    run(
        &mut sut,
        &mut world,
        ask(
            &account,
            "m1",
            "personal_sign",
            json!(["0x68656c6c6f", account.address]),
        ),
    );
    let held = sut.view().dapp["m1"].clone();
    assert_eq!(held.stage, HeldStage::Refused);
    assert_eq!(held.answer, None, "said first");
    assert!(sut.view().draft.is_none());
    run(&mut sut, &mut world, Event::DappLeave { id: "m1".into() });
    let error = answer(&sut, "m1").unwrap().error.unwrap();
    assert_eq!(error.code, 4200);
    assert!(error.message.contains("signs no messages"));
}

/// The core reads what the site asks, refused as a Vela account's would be.
#[test]
fn a_request_the_core_cannot_read_is_answered_at_once() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(
        &mut sut,
        &mut world,
        ask(
            &account,
            "x",
            "eth_sendTransaction",
            json!([{ "from": account.address, "value": "0x1" }]),
        ),
    );
    let error = answer(&sut, "x").unwrap().error.unwrap();
    assert_eq!(error.code, -32602);
    assert!(sut.view().draft.is_none());
}

// -- core-ms-2 -----------------------------------------------------------------------------

/// A site's transaction the proposer's own approval makes ready (a 1-of-1):
/// "Approve and execute" — proposed, then executed by the same member, and
/// the site answered the transaction that ran it.
#[test]
fn a_site_transaction_ready_on_its_proposal_runs_in_one_press() {
    let me = fixture(0).address;
    let g = genesis(&[me.as_str()], 1, "0x3a");
    let account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let mut sut = Sut::new();
    let ev = Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(&mut sut, &mut world, ev);
    run(&mut sut, &mut world, send_tx(&account));
    let draft = sut.view().draft.clone().unwrap();
    assert!(draft.execute_now, "its own approval makes it ready");
    let hash = draft.item.safe_tx_hash.clone();
    let mut after = DeployedSafe::clean(&[me.as_str()], 1);
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
    run_short(
        &mut sut,
        &mut world,
        Event::ProposeExecute { as_member: None },
    );
    assert_eq!(world.sent.len(), 1, "{:?}", sut.view().actions);
    assert!(matches!(
        sut.view().actions[&format!("execute:{}", hash.to_ascii_lowercase())],
        ActionState::Done { .. }
    ));
    assert_eq!(answer(&sut, "r1").unwrap().result, Some(json!(tx_hash)));
}

/// A 2-of-3's site transaction needs another approval: proposed only.
#[test]
fn a_site_transaction_short_of_approvals_is_not_offered_the_one_press() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    run(&mut sut, &mut world, send_tx(&account));
    assert!(!sut.view().draft.unwrap().execute_now);
}

/// `wallet_getCallsStatus` answers go once taken, and those not taken are
/// kept at most `CALLS_STATUS_KEPT`, the oldest going first.
#[test]
fn calls_status_answers_are_cleared_once_taken_and_bounded() {
    use vela_core::app::multisig_proposals::flows::CALLS_STATUS_KEPT;
    let (mut world, mut sut, _) = setup(&[0]);
    let unknown = format!("0x{}", "12".repeat(32));
    for i in 0..=CALLS_STATUS_KEPT {
        run(
            &mut sut,
            &mut world,
            Event::CallsStatus {
                id: format!("s{i}"),
                chain_id: 100,
                calls_id: unknown.clone(),
            },
        );
    }
    let kept = sut.view().calls_status;
    assert_eq!(kept.len(), CALLS_STATUS_KEPT);
    assert!(!kept.contains_key("s0"), "the oldest went");
    run(
        &mut sut,
        &mut world,
        Event::CallsStatusTaken { id: "s3".into() },
    );
    assert!(!sut.view().calls_status.contains_key("s3"));
    assert_eq!(sut.view().calls_status.len(), CALLS_STATUS_KEPT - 1);
}

// ---------------------------------------------------------------------------
// §31 — a site may never make a multisig act for a Safe it owns
// ---------------------------------------------------------------------------

/// `approveHash(0xcd…)` — one counting approval of `safe`.
fn approve_hash_on(account: &MultisigAccount, safe: &str) -> Event {
    ask(
        account,
        "r1",
        "eth_sendTransaction",
        json!([{ "from": account.address, "to": safe, "value": "0x0",
                 "data": format!("0xd4d9bdcd{}", "cd".repeat(32)) }]),
    )
}

/// A 1-of-1 parent whose one member is `account`.
fn parent_of(account: &MultisigAccount) -> MultisigAccount {
    let g = genesis(&[account.address.as_str()], 1, "0x77");
    serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap()
}

fn refusal_data(sut: &Sut, id: &str) -> Value {
    let held = sut.view().dapp[id].clone();
    assert_eq!(held.stage, HeldStage::Refused, "{held:?}");
    held.refusal.unwrap().data.unwrap()
}

#[test]
fn a_site_asking_a_multisig_to_approve_for_a_multisig_on_the_list_is_refused() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let parent = parent_of(&account);
    run(
        &mut sut,
        &mut world,
        Event::Accounts {
            accounts: vec![account.clone(), parent.clone()],
        },
    );
    run(
        &mut sut,
        &mut world,
        approve_hash_on(&account, &parent.address),
    );
    let data = refusal_data(&sut, "r1");
    assert_eq!(data["reason"], "parent_safe");
    assert_eq!(data["why"], "member");
    assert_eq!(data["function"], "approveHash");
    assert_eq!(data["key"], "componentsUi.signing.parentSafeBlocked");
    assert!(sut.view().draft.is_none(), "nothing drafted");
    assert!(world.passkeys.is_empty(), "nothing signed");
    // Read, then answered with the refusal — never 4001.
    run(&mut sut, &mut world, Event::DappLeave { id: "r1".into() });
    let answer = answer(&sut, "r1").unwrap();
    assert_eq!(answer.error.unwrap().code, -32603);
}

#[test]
fn a_safe_the_chain_says_the_multisig_owns_is_refused_after_the_read() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let parent = parent_of(&account);
    world.safes.insert(
        (100, parent.address.to_ascii_lowercase()),
        DeployedSafe::clean(&[account.address.as_str()], 1),
    );
    run(
        &mut sut,
        &mut world,
        approve_hash_on(&account, &parent.address),
    );
    assert!(
        world
            .rpc_log
            .iter()
            .any(|(_, method, params)| method == "eth_call"
                && params[0]["data"]
                    .as_str()
                    .is_some_and(|d| d.starts_with("0x2f54bf6e"))),
        "isOwner was read"
    );
    assert_eq!(refusal_data(&sut, "r1")["why"], "owner");
    assert!(sut.view().draft.is_none());
}

#[test]
fn a_safe_the_multisig_does_not_own_is_drafted_once_read() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let other = genesis(&[fixture(2).address.as_str()], 1, "0x78");
    world.safes.insert(
        (100, other.address.to_ascii_lowercase()),
        DeployedSafe::clean(&[fixture(2).address.as_str()], 1),
    );
    run(
        &mut sut,
        &mut world,
        approve_hash_on(&account, &other.address),
    );
    let view = sut.view();
    assert_eq!(view.dapp["r1"].stage, HeldStage::Reviewing);
    assert!(view.draft.is_some(), "read false: its review, as any call");
}

#[test]
fn an_owner_read_nobody_could_make_refuses() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let target = "0x7777777777777777777777777777777777777777";
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        (method == "eth_call"
            && params[0]["to"]
                .as_str()
                .is_some_and(|to| to.eq_ignore_ascii_case(target)))
        .then(
            || vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "offline".into(),
            },
        )
    }));
    run(&mut sut, &mut world, approve_hash_on(&account, target));
    let data = refusal_data(&sut, "r1");
    assert_eq!(data["why"], "unchecked");
    assert_eq!(data["key"], "componentsUi.signing.parentSafeUnchecked");
}

// ---------------------------------------------------------------------------
// §35 (SEC-PARENT-2) — a site may never make a multisig call itself to change
// who controls it
// ---------------------------------------------------------------------------

const SELF_CALL: &str = include_str!("vectors/rules/multisig-self-call.json");

/// The vector's calls with `SELF` as `multisig`'s address.
fn self_calls(case: &Value, multisig: &str) -> Value {
    let bare = multisig.trim_start_matches("0x").to_ascii_lowercase();
    let calls: Vec<Value> = case["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            let to = match call["to"].as_str().unwrap() {
                "SELF" => multisig.to_owned(),
                "SELF_UPPER" => format!("0x{}", bare.to_ascii_uppercase()),
                other => other.to_owned(),
            };
            json!({
                "to": to,
                "value": call["value"],
                "data": call["data"].as_str().unwrap().replace("SELFADDR", &bare),
            })
        })
        .collect();
    Value::Array(calls)
}

#[test]
fn the_multisig_self_call_vector_is_refused_at_arrival_and_nothing_is_drafted() {
    let vector: Value = serde_json::from_str(SELF_CALL).unwrap();
    for case in vector["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let (mut world, mut sut, account) = setup(&[0, 1]);
        let calls = self_calls(case, &account.address);
        run(
            &mut sut,
            &mut world,
            ask(
                &account,
                "r1",
                "wallet_sendCalls",
                json!([{ "version": "2.0.0", "chainId": "0x64", "from": account.address,
                         "calls": calls }]),
            ),
        );
        let held = sut.view().dapp["r1"].clone();
        match case["refused"].as_str() {
            Some(function) => {
                assert_eq!(held.stage, HeldStage::Refused, "{name}: {held:?}");
                let refusal = held.refusal.clone().unwrap();
                assert_eq!(refusal.code, -32603, "{name}");
                let data = refusal.data.unwrap();
                assert_eq!(data["reason"], "self_call", "{name}");
                assert_eq!(data["function"], function, "{name}");
                assert_eq!(data["key"], vector["key"], "{name}");
                assert_eq!(data["safe"], json!(account.address), "{name}");
                assert_eq!(data["leg"], case["leg"], "{name}");
                assert!(sut.view().draft.is_none(), "{name}: nothing drafted");
                assert!(world.passkeys.is_empty(), "{name}: nothing signed");
                // Read, then answered with the refusal — never 4001.
                run(&mut sut, &mut world, Event::DappLeave { id: "r1".into() });
                let answer = answer(&sut, "r1").unwrap();
                assert_eq!(answer.error.unwrap().code, -32603, "{name}");
            }
            None => {
                assert_eq!(held.stage, HeldStage::Reviewing, "{name}: {held:?}");
                assert!(sut.view().draft.is_some(), "{name}: drafted");
            }
        }
    }
}

/// The proposal step checks again, on the transaction about to be signed: a
/// site's calls that reached a draft without the arrival check (the
/// `Draft` event carries a site's `Calls` too) are never proposed — the
/// held request, if any, refused in the arrival's words.
#[test]
fn a_site_draft_that_would_change_the_multisig_is_never_proposed() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    let add_owner = format!(
        "0x0d582f13{:0>64}{:0>64}",
        DEAD.trim_start_matches("0x").to_ascii_lowercase(),
        "1"
    );
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: vela_core::app::multisig_proposals::ProposalKind::Calls {
                calls: vec![multisig::CallArg {
                    to: account.address.clone(),
                    value: "0x0".into(),
                    data: add_owner,
                }],
                origin: Some("https://app.example".into()),
            },
        },
    );
    assert!(sut.view().draft.is_some(), "a draft, by the other door");
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(
        sut.view().actions.get("propose"),
        Some(&ActionState::SigningBlocked)
    );
    assert!(world.passkeys.is_empty(), "nothing signed");
    assert!(world.mailbox.is_empty(), "nothing proposed");
}

/// The members' own change of members is not a site's request: proposed.
#[test]
fn the_members_own_change_of_members_is_still_proposed() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
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
            proposal: vela_core::app::multisig_proposals::ProposalKind::MemberChange {
                change: multisig::MemberChange::Threshold { threshold: 1 },
            },
        },
    );
    run_short(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(
        matches!(
            sut.view().actions.get("propose"),
            Some(ActionState::Done { .. })
        ),
        "{:?}",
        sut.view().actions.get("propose")
    );
}
