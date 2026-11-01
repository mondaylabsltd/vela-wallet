//! The outside-owner machines (`multisig_outside`), driven against a
//! simulated shell — the web's `external/outside`, `session`, `bridge`,
//! `relay`, `typed`, `onchain`, `errors` suites, `core/signers`, the answer
//! rule of `core/page-approval` and the logic of `ui/ExternalSignSheet`,
//! rule for rule; and one run with both sides of the channel connected.
//!
//! The real core throughout: the chain holds Safe's own v1.4.1 code, the
//! owners are EOAs with k256 keys, the wallet signs typed data by its own
//! EIP-712 hash and `personal_sign` by EIP-191.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_outside::page::{
    self, ApprovalForm, FailReason, Outcome, OutsideAction, OutsideLoad, OutsideStep, PageGate,
    Stage,
};
use vela_core::app::multisig_outside::session::{
    self, Failure, Phase, SessionOutcome, SessionRequest,
};
use vela_core::app::multisig_outside::{
    self as outside, relay, ItemStatus, RefusalKind, SignerFacts, SignerKind, TxLink, WalletProblem,
};
use vela_core::app::multisig_shell::{MultisigOperation as Op, MultisigShellResult as Res};
use vela_core::multisig::{self, EcdsaApproval, EcdsaForm, SafeTx};
use world::{eoa, eoa_sign, settle_ready, DeployedSafe, WalletSim, World, FACTORY, SERVICE};

type Page = DomainDriver<page::OutsidePage>;
type Session = DomainDriver<session::OutsideSession>;

const CHAIN: u64 = 100;
const SAFE: &str = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";
const MODULE: &str = "0x9813461E88aDD60D0eC0943B74A83368CA2E36D1";
const ORIGIN: &str = "https://wallet.getvela.app";
const SEED_A: u8 = 7;
const SEED_B: u8 = 8;
const SEED_STRANGER: u8 = 9;

fn addr(seed: u8) -> String {
    eoa(seed).1
}

fn sign_hash(seed: u8, hash: &str) -> String {
    eoa_sign(
        &eoa(seed).0,
        &vela_core::primitives::from_hex(hash).unwrap(),
    )
}

fn tx(nonce: &str) -> SafeTx {
    SafeTx {
        to: "0x000000000000000000000000000000000000dEaD".into(),
        value: "0x2386f26fc10000".into(),
        data: "0x".into(),
        operation: 0,
        safe_tx_gas: "0x0".into(),
        base_gas: "0x0".into(),
        gas_price: "0x0".into(),
        gas_token: "0x0000000000000000000000000000000000000000".into(),
        refund_receiver: "0x0000000000000000000000000000000000000000".into(),
        nonce: nonce.into(),
    }
}

fn hash_of(safe: &str, t: &SafeTx) -> String {
    multisig::safe_tx_hash(CHAIN, safe, t).unwrap()
}

fn approval_by(seed: u8, hash: &str) -> Value {
    json!({ "owner": addr(seed), "signature": sign_hash(seed, hash), "createdMs": 1 })
}

fn stored(safe: &str, t: &SafeTx, approvals: Vec<Value>) -> Value {
    json!({
        "chainId": CHAIN, "safe": safe, "safeTxHash": hash_of(safe, t), "safeTx": t,
        "proposer": addr(SEED_A), "approvals": approvals, "createdMs": 1, "updatedMs": 1,
    })
}

fn link_to(safe: &str, t: &SafeTx) -> TxLink {
    TxLink {
        chain_id: CHAIN,
        safe: safe.to_owned(),
        safe_tx_hash: hash_of(safe, t),
        for_members: Vec::new(),
    }
}

/// A deployed 2-of-2 (A, B) at nonce 3; the mailbox holds A's approval of
/// the transfer; the wallet holds B.
fn deployed_world() -> World {
    let mut w = World::default();
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &addr(SEED_B)], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let t = tx("0x3");
    w.mailbox = vec![stored(
        SAFE,
        &t,
        vec![approval_by(SEED_A, &hash_of(SAFE, &t))],
    )];
    w.wallet = Some(WalletSim::holding(&[SEED_B], CHAIN));
    chain_answers(&mut w);
    w
}

/// The same multisig not deployed yet: its genesis on record (A, B, 2 of 2).
fn undeployed_world() -> (World, String) {
    let mut w = World::default();
    let g = world::genesis(&[&addr(SEED_A), &addr(SEED_B)], 2, "0x05");
    w.record(&g.address, &g.genesis, false);
    let t = tx("0x0");
    w.mailbox = vec![stored(
        &g.address,
        &t,
        vec![approval_by(SEED_A, &hash_of(&g.address, &t))],
    )];
    w.wallet = Some(WalletSim::holding(&[SEED_B], CHAIN));
    chain_answers(&mut w);
    (w, g.address)
}

/// The chain's answers this world does not hold itself: fees, receipts, coin.
fn chain_answers(w: &mut World) {
    w.rpc_hooks.push(Box::new(|_, method, _| {
        let ok = |v: Value| Some(Res::RpcResult { result: v });
        match method {
            "eth_gasPrice" => ok(json!("0x3b9aca00")),
            "eth_estimateGas" => ok(json!("0x5208")),
            "eth_getTransactionReceipt" => ok(json!({ "status": "0x1" })),
            "eth_getBalance" => ok(json!("0x2386f26fc10000")),
            _ => None,
        }
    }));
}

fn run<A>(sut: &mut DomainDriver<A>, w: &mut World, event: A::Event)
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: vela_core::app::SplitEffect<Op = Op>,
{
    sut.dispatch(event);
    let answered = settle_ready(sut, w, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

fn opened(w: &mut World, link: TxLink) -> Page {
    let mut sut = Page::new();
    run(
        &mut sut,
        w,
        page::Event::Start {
            service_url: SERVICE.to_owned(),
            wallet_name: "Rabby".to_owned(),
        },
    );
    run(&mut sut, w, page::Event::Link { link });
    sut
}

/// A world whose injected wallet holds these seeds' keys.
fn with_wallet(seeds: &[u8]) -> World {
    World {
        wallet: Some(WalletSim::holding(seeds, CHAIN)),
        ..World::default()
    }
}

fn wallet(w: &mut World) -> &mut WalletSim {
    w.wallet.as_mut().unwrap()
}

/// The approvals the mailbox holds for `hash`.
fn mailbox_signatures(w: &World, hash: &str) -> Vec<(String, String)> {
    w.mailbox
        .iter()
        .filter(|p| p["safeTxHash"].as_str().unwrap().eq_ignore_ascii_case(hash))
        .flat_map(|p| p["approvals"].as_array().unwrap().clone())
        .map(|a| {
            (
                a["owner"].as_str().unwrap().to_ascii_lowercase(),
                a["signature"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

fn refuse_with(w: &mut World, refusals: Vec<(u16, Value)>) {
    let queue = Rc::new(RefCell::new(VecDeque::from(refusals)));
    w.submit_hooks.push(Box::new(move |_| {
        queue
            .borrow_mut()
            .pop_front()
            .map(|(status, body)| (status, body, None))
    }));
}

// ---------------------------------------------------------------------------
// The connect page: reading the transaction from the mailbox alone (r4C-2)
// ---------------------------------------------------------------------------

#[test]
fn the_links_transaction_its_approvals_verified_and_counted_by_the_chains_owners() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let sut = opened(&mut w, link_to(SAFE, &t));
    let view = sut.view();
    assert_eq!(view.load, OutsideLoad::Ready, "{:?}", view.load);
    let v = view.tx.unwrap();
    assert_eq!(v.item.approved, 1);
    assert_eq!(v.hashes.safe_tx_hash, hash_of(SAFE, &t));
    assert_eq!(v.gate, PageGate::Open);
    assert!(v.review.is_some(), "the one review is drawn on the page");
    assert!(v.can_approve(&addr(SEED_B)));
    assert!(!v.can_approve(&addr(SEED_A)), "A has approved");
    assert!(!v.can_approve(&addr(SEED_STRANGER)));
    // B executing counts as the second approval (msg.sender).
    assert!(v.can_execute(&addr(SEED_B)));
    assert!(!v.can_execute(&addr(SEED_STRANGER)));
}

#[test]
fn what_the_mailbox_holds_under_the_hash_must_be_that_transaction() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut other = stored(SAFE, &t, vec![]);
    other["safeTx"]["value"] = json!("0x1");
    w.mailbox = vec![other];
    let sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().load, OutsideLoad::Mismatch);
    assert!(sut.view().tx.is_none());
}

#[test]
fn an_approval_in_the_mailbox_that_does_not_verify_is_not_counted() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    w.mailbox = vec![stored(
        SAFE,
        &t,
        vec![
            json!({ "owner": addr(SEED_A), "signature": sign_hash(SEED_STRANGER, &hash), "createdMs": 1 }),
        ],
    )];
    let sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert_eq!(v.item.approved, 0);
    assert!(!v.item.approvals[0].counts);
    assert!(!v.can_execute(&addr(SEED_B)));
}

#[test]
fn the_mailbox_lacking_it_is_said_as_missing_and_an_unreachable_one_as_away() {
    let mut w = deployed_world();
    w.mailbox.clear();
    let t = tx("0x3");
    let sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().load, OutsideLoad::Missing);

    let mut w = deployed_world();
    w.mailbox_down.insert(CHAIN);
    let sut = opened(&mut w, link_to(SAFE, &t));
    assert!(matches!(
        sut.view().load,
        OutsideLoad::Unavailable { ref what, .. } if what == "mailbox"
    ));
}

#[test]
fn approvals_meeting_the_threshold_leave_only_executing() {
    let mut w = World::default();
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &addr(SEED_B), &addr(SEED_STRANGER)], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    w.mailbox = vec![stored(
        SAFE,
        &t,
        vec![
            approval_by(SEED_A, &hash),
            approval_by(SEED_STRANGER, &hash),
        ],
    )];
    w.wallet = Some(WalletSim::holding(&[SEED_B], CHAIN));
    let sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert_eq!(v.item.approved, 2);
    assert!(v.enough());
    assert!(!v.can_approve(&addr(SEED_B)), "nothing more to sign");
    assert!(v.can_execute(&addr(SEED_B)));
}

/// r4C-16: an owner that approved on chain (`approveHash`) counts — read
/// from the chain, never the mailbox; a yes is kept, a no asked again later.
#[test]
fn approvals_on_chain_count_a_yes_kept_a_no_asked_again_later() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    w.mailbox = vec![stored(SAFE, &t, vec![])];
    w.safe_mut(CHAIN, SAFE)
        .approved
        .insert((addr(SEED_A), hash.to_ascii_lowercase()));
    let asked = |w: &World| {
        w.rpc_log
            .iter()
            .filter(|(_, m, p)| {
                m == "eth_call"
                    && p[0]["data"]
                        .as_str()
                        .unwrap_or("")
                        .starts_with("0x7d832974")
            })
            .count()
    };
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert_eq!(v.item.approved, 1);
    assert!(matches!(
        v.item.approvals[0].approval,
        multisig::Approval::OnChain(_)
    ));
    assert_eq!(asked(&w), 2, "A and B asked once");
    run(&mut sut, &mut w, page::Event::Refresh);
    assert_eq!(asked(&w), 2, "soon after: nothing asked");
    w.now += outside::RECHECK_MS + 1.0;
    run(&mut sut, &mut w, page::Event::Refresh);
    assert_eq!(asked(&w), 3, "only the no is asked again");
}

#[test]
fn a_chain_that_does_not_answer_for_approved_hashes_approves_nobody() {
    let mut w = deployed_world();
    let t = tx("0x3");
    w.mailbox = vec![stored(SAFE, &t, vec![])];
    w.rpc_hooks.insert(
        0,
        Box::new(|_, m, p| {
            (m == "eth_call"
                && p[0]["data"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("0x7d832974"))
            .then(|| Res::RpcUnanswered {
                detail: "down".into(),
            })
        }),
    );
    let sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().tx.unwrap().item.approved, 0);
}

// ---------------------------------------------------------------------------
// Approving from the page (r4C-2, r4C-16)
// ---------------------------------------------------------------------------

#[test]
fn approving_signs_typed_data_checks_it_and_hands_it_to_the_mailbox() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::TypedData
        }
    );
    let posted = mailbox_signatures(&w, &hash);
    let (owner, signature) = posted.iter().find(|(o, _)| *o == addr(SEED_B)).unwrap();
    let checked = multisig::verify_ecdsa_approval(
        &hash,
        &EcdsaApproval {
            owner: owner.clone(),
            signature: signature.clone(),
        },
    )
    .unwrap();
    assert!(checked.owner.eq_ignore_ascii_case(&addr(SEED_B)));
    assert_eq!(
        sut.view().tx.unwrap().item.approved,
        2,
        "read again from the mailbox"
    );
}

#[test]
fn a_signature_that_is_not_the_owners_is_never_handed_on() {
    let mut w = deployed_world();
    w.wallet = Some(WalletSim::holding(&[SEED_B, SEED_STRANGER], CHAIN));
    wallet(&mut w).signer = Some(addr(SEED_STRANGER));
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            reason: Some(FailReason::BadSignature),
            ..
        }
    ));
    assert!(w.submits.is_empty());
}

#[test]
fn a_wallet_without_typed_data_is_offered_safes_eth_sign_form() {
    let mut w = deployed_world();
    wallet(&mut w).typed = false;
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(sut.view().step, OutsideStep::Unsupported);
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: Some(EcdsaForm::EthSign),
        },
    );
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::EthSign
        }
    );
    let (_, sig) = mailbox_signatures(&w, &hash)
        .into_iter()
        .find(|(o, _)| *o == addr(SEED_B))
        .unwrap();
    assert!(sig.ends_with("1f") || sig.ends_with("20"), "{sig}");
}

#[test]
fn a_mailbox_that_refuses_the_hash_form_is_said_as_that() {
    let mut w = deployed_world();
    wallet(&mut w).typed = false;
    w.submit_hooks.push(Box::new(|req| {
        let sig = req["approval"]["signature"].as_str().unwrap_or("");
        (sig.ends_with("1f") || sig.ends_with("20")).then(|| {
            (
                422,
                json!({ "refusal": { "kind": "approval", "error": { "kind": "notAnEip712Signature" } } }),
                None,
            )
        })
    }));
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: Some(EcdsaForm::EthSign),
        },
    );
    assert_eq!(
        sut.view().step,
        OutsideStep::Failed {
            what: OutsideAction::Approve,
            problem: None,
            reason: Some(FailReason::NoEthSign),
            refused: None,
            unsent: false,
        },
        "nothing kept: it never will take this form"
    );
    assert!(!w.store.contains_key(outside::KEPT_KEY));
}

#[test]
fn a_busy_mailbox_is_waited_out_and_sent_again_nothing_signed_twice() {
    let mut w = deployed_world();
    let busy = Rc::new(RefCell::new(1));
    w.submit_hooks.push(Box::new(move |_| {
        let mut left = busy.borrow_mut();
        (*left > 0).then(|| {
            *left -= 1;
            (429, json!({ "error": "busy" }), Some("1".to_owned()))
        })
    }));
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::TypedData
        }
    );
    assert_eq!(w.submits.len(), 2, "sent again after the wait it named");
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 1);
    assert_eq!(mailbox_signatures(&w, &hash).len(), 2);
}

#[test]
fn a_mailbox_away_for_two_minutes_is_said_as_not_sent_and_kept() {
    let mut w = deployed_world();
    w.submit_hooks
        .push(Box::new(|_| Some((429, json!({ "error": "busy" }), None))));
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    let start = w.now;
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            reason: Some(FailReason::NotSent),
            unsent: true,
            ..
        }
    ));
    assert!(w.now - start <= outside::POST_PATIENCE_MS + 1_000.0);
    assert!(w.store[outside::KEPT_KEY].contains(&addr(SEED_B)));
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 1);
}

#[test]
fn a_declined_signature_is_said_as_declined_in_the_wallet() {
    let mut w = deployed_world();
    wallet(&mut w).next.push_back((
        "eth_signTypedData_v4".into(),
        Res::WalletRefused {
            code: Some(4001),
            message: "User rejected".into(),
        },
    ));
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            problem: Some(WalletProblem::Declined),
            ..
        }
    ));
}

#[test]
fn the_wallet_is_moved_to_the_transactions_network_first_and_a_stay_is_said() {
    let mut w = deployed_world();
    wallet(&mut w).chain_id = 1;
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(wallet(&mut w).calls_of("wallet_switchEthereumChain"), 1);
    assert!(matches!(sut.view().step, OutsideStep::Approved { .. }));

    let mut w = deployed_world();
    wallet(&mut w).chain_id = 1;
    wallet(&mut w).next.push_back((
        "wallet_switchEthereumChain".into(),
        Res::WalletAnswered {
            result: Value::Null,
        },
    ));
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            problem: Some(WalletProblem::OtherChain),
            ..
        }
    ));
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
}

#[test]
fn a_wallet_that_does_not_answer_can_be_stopped_waiting_on_never_forever() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    wallet(&mut w).hold = true;
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    let step = sut.view().step;
    assert!(matches!(
        step,
        OutsideStep::Wallet { .. } | OutsideStep::Switching { .. } | OutsideStep::Checking { .. }
    ));
    // In the wallet this long: said to be waiting, with Cancel and Ask again.
    let asking = OutsideStep::Wallet {
        what: OutsideAction::Approve,
        since: w.now,
        index: 1,
        total: 1,
    };
    assert!(!asking.stalled(w.now + 1_000.0));
    assert!(asking.stalled(w.now + session::STILL_WAITING_MS + 1.0));
    run(&mut sut, &mut w, page::Event::Cancel);
    assert_eq!(sut.view().step, OutsideStep::Idle);
    // The wallet answers late: nothing is handed on for a cancelled ask.
    wallet(&mut w).hold = false;
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().step, OutsideStep::Idle);
    assert!(w.submits.is_empty());
}

// ---------------------------------------------------------------------------
// Executing and rejecting from the page (r4C-9, r5reg-13)
// ---------------------------------------------------------------------------

#[test]
fn executing_with_the_owners_own_wallet_after_its_fee_is_said() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Execute {
            account: addr(SEED_B),
        },
    );
    assert!(
        wallet(&mut w).sent.is_empty(),
        "the fee first, then the tap"
    );
    let fee = sut.view().fee.unwrap();
    assert_eq!(
        fee.fee.unwrap(),
        page::ExecutionFee {
            wei: (21_000u64 * 1_000_000_000).to_string(),
            count: 1,
            partial: false,
            // r8-15: the most it can take, said beside the estimate.
            max_wei: (21_000u64 * 1_000_000_000 * 2).to_string(),
        }
    );
    run(
        &mut sut,
        &mut w,
        page::Event::Execute {
            account: addr(SEED_B),
        },
    );
    assert_eq!(
        sut.view().step,
        OutsideStep::Executed {
            tx_hash: format!("0x{}", "77".repeat(32))
        }
    );
    let sent = wallet(&mut w).sent.clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0]["to"].as_str().unwrap().eq_ignore_ascii_case(SAFE));
    assert_eq!(sent[0]["from"], json!(addr(SEED_B)));
    let data = sent[0]["data"].as_str().unwrap();
    assert!(data.starts_with("0x6a761202"), "execTransaction");
    // A's own signature, and B as msg.sender (v = 1).
    assert!(data.contains(&sign_hash(SEED_A, &hash)[2..]));
}

/// r7-3: approvals that already meet the threshold are what it carries —
/// the executor only sends it, never approving as `msg.sender` in place of
/// one given (which would then read as "not counted").
#[test]
fn approvals_already_enough_are_carried_and_the_executor_only_sends_it() {
    let mut w = World::default();
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &addr(SEED_B), &addr(SEED_STRANGER)], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    w.mailbox = vec![stored(
        SAFE,
        &t,
        vec![
            approval_by(SEED_A, &hash),
            approval_by(SEED_STRANGER, &hash),
        ],
    )];
    w.wallet = Some(WalletSim::holding(&[SEED_B], CHAIN));
    chain_answers(&mut w);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().tx.unwrap().item.approved, 2);
    for _ in 0..2 {
        run(
            &mut sut,
            &mut w,
            page::Event::Execute {
                account: addr(SEED_B),
            },
        );
    }
    assert!(matches!(sut.view().step, OutsideStep::Executed { .. }));
    let sent = wallet(&mut w).sent.clone();
    assert_eq!(sent.len(), 1);
    let data = sent[0]["data"].as_str().unwrap().to_ascii_lowercase();
    assert!(data.contains(&sign_hash(SEED_A, &hash)[2..].to_ascii_lowercase()));
    assert!(data.contains(&sign_hash(SEED_STRANGER, &hash)[2..].to_ascii_lowercase()));
    let as_sender = format!(
        "{:0>64}{}01",
        addr(SEED_B)[2..].to_ascii_lowercase(),
        "0".repeat(64)
    );
    assert!(!data.contains(&as_sender), "no msg.sender approval");
}

#[test]
fn rejecting_proposes_the_same_number_cancellation_approved_by_the_owner() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    assert!(sut.view().tx.unwrap().can_reject(&addr(SEED_B)));
    run(
        &mut sut,
        &mut w,
        page::Event::Reject {
            account: addr(SEED_B),
        },
    );
    assert_eq!(sut.view().step, OutsideStep::Rejected);
    assert_eq!(w.submits.len(), 1);
    let cancel: SafeTx = serde_json::from_value(w.submits[0]["safeTx"].clone()).unwrap();
    assert!(cancel.to.eq_ignore_ascii_case(SAFE));
    assert_eq!(cancel.nonce, "0x3");
    let approval: EcdsaApproval = serde_json::from_value(w.submits[0]["approval"].clone()).unwrap();
    assert!(multisig::verify_ecdsa_approval(&hash_of(SAFE, &cancel), &approval).is_ok());
}

#[test]
fn the_wallets_coin_which_pays_the_fee_is_read() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Balance {
            account: addr(SEED_B),
        },
    );
    assert_eq!(
        sut.view().balance.unwrap().wei.as_deref(),
        Some("10000000000000000")
    );
}

// ---------------------------------------------------------------------------
// Who can act for it on this network (§3.5) and the review (§3.6)
// ---------------------------------------------------------------------------

#[test]
fn a_refused_genesis_is_read_only_nothing_approved_executed_or_rejected() {
    let (mut w, safe) = undeployed_world();
    w.canonical_missing
        .insert((CHAIN, FACTORY.to_ascii_lowercase()));
    let t = tx("0x0");
    let mut sut = opened(&mut w, link_to(&safe, &t));
    let v = sut.view().tx.unwrap();
    assert!(matches!(v.gate, PageGate::Refused { .. }), "{:?}", v.gate);
    let b = addr(SEED_B);
    assert!(!v.can_approve(&b) && !v.can_execute(&b) && !v.can_reject(&b));
    for event in [
        page::Event::Approve {
            account: b.clone(),
            form: None,
        },
        page::Event::Execute { account: b.clone() },
        page::Event::Reject { account: b.clone() },
    ] {
        run(&mut sut, &mut w, event);
    }
    assert!(w.submits.is_empty());
    assert!(wallet(&mut w).log.is_empty(), "the wallet is never asked");
}

#[test]
fn not_checkable_yet_nothing_acts() {
    let mut w = deployed_world();
    w.rpc_hooks.insert(
        0,
        Box::new(|_, m, _| {
            (m == "eth_getStorageAt").then(|| Res::RpcUnanswered {
                detail: "timed out".into(),
            })
        }),
    );
    let t = tx("0x3");
    let sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert_eq!(v.gate, PageGate::Unchecked);
    assert!(!v.can_approve(&addr(SEED_B)));
}

#[test]
fn a_risk_is_said_and_each_signature_asks_first() {
    let mut w = deployed_world();
    w.safe_mut(CHAIN, SAFE).modules = vec![MODULE.to_owned()];
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert!(matches!(v.gate, PageGate::Risk { .. }));
    assert!(v.can_approve(&addr(SEED_B)), "the owner may still act");
    // "Don't sign": nothing asked of the wallet.
    w.risk_signs = false;
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(w.risks_asked.len(), 1);
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
    assert_eq!(sut.view().step, OutsideStep::Idle);
    // Asked again — never remembered — and signed on the person's yes.
    w.risk_signs = true;
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(w.risks_asked.len(), 2);
    assert!(matches!(sut.view().step, OutsideStep::Approved { .. }));
}

/// §3.6: who can act for it is read again right before the signature — a
/// module added after the page loaded is caught before the wallet is asked.
#[test]
fn the_chain_is_read_again_before_every_signature() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().tx.unwrap().gate, PageGate::Open);
    w.safe_mut(CHAIN, SAFE).modules = vec![MODULE.to_owned()];
    w.risk_signs = false;
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert_eq!(w.risks_asked.len(), 1, "the new module is said");
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
}

/// A transaction the core does not sign (a delegatecall to an unknown
/// contract, Bybit's shape): blocked here, no override, the wallet never asked.
#[test]
fn a_transaction_the_core_does_not_sign_is_blocked() {
    let mut w = deployed_world();
    let mut t = tx("0x3");
    t.operation = 1;
    t.to = "0x00000000000000000000000000000000bad1bad1".into();
    t.value = "0x0".into();
    w.mailbox = vec![stored(
        SAFE,
        &t,
        vec![approval_by(SEED_A, &hash_of(SAFE, &t))],
    )];
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    let v = sut.view().tx.unwrap();
    assert_eq!(v.gate, PageGate::Blocked);
    assert!(v.item.review.action.is_none());
    assert!(!v.can_approve(&addr(SEED_B)) && !v.can_execute(&addr(SEED_B)));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(wallet(&mut w).log.is_empty());
}

#[test]
fn a_deploy_from_a_genesis_the_code_read_just_then_does_not_verify_is_never_sent() {
    let (mut w, safe) = undeployed_world();
    let t = tx("0x0");
    let mut sut = opened(&mut w, link_to(&safe, &t));
    let v = sut.view().tx.unwrap();
    assert!(v.genesis.is_some());
    assert!(v.can_execute(&addr(SEED_B)));
    run(
        &mut sut,
        &mut w,
        page::Event::EstimateFee {
            account: addr(SEED_B),
        },
    );
    assert_eq!(
        sut.view().fee.unwrap().fee.unwrap().count,
        2,
        "deployed first"
    );
    w.canonical_missing
        .insert((CHAIN, FACTORY.to_ascii_lowercase()));
    run(
        &mut sut,
        &mut w,
        page::Event::Execute {
            account: addr(SEED_B),
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            reason: Some(FailReason::DeployRefused),
            ..
        }
    ));
    assert!(wallet(&mut w).sent.is_empty());
}

#[test]
fn approving_on_chain_sends_approve_hash_from_the_wallet() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    assert!(sut.view().actions.is_none(), "no wallet connected yet");
    run(&mut sut, &mut w, page::Event::ConnectWallet);
    assert!(sut.view().actions.unwrap().can_approve_on_chain);
    run(
        &mut sut,
        &mut w,
        page::Event::ApproveOnChain {
            account: addr(SEED_B),
        },
    );
    let sent = wallet(&mut w).sent.clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["data"], json!(outside::approve_hash_call(&hash)));
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::OnChain
        }
    );
    assert!(
        w.submits.is_empty(),
        "an approval on chain never goes to the mailbox"
    );
}

// ---------------------------------------------------------------------------
// A refusal never loses what was signed (r5reg-12)
// ---------------------------------------------------------------------------

#[test]
fn the_mailboxs_refusal_in_one_line_and_by_kind() {
    use outside::{refusal_kind, refused_form, short_refusal};
    assert_eq!(
        short_refusal(&json!({ "kind": "transaction", "detail": "delegatecall is refused" })),
        "delegatecall is refused"
    );
    assert_eq!(
        short_refusal(&json!({ "kind": "approval", "error": { "kind": "notAnOwner" } })),
        "notAnOwner"
    );
    assert_eq!(short_refusal(&json!({ "kind": "full" })), "full");
    assert!(refused_form(
        &json!({ "kind": "approval", "error": { "kind": "notAnEip712Signature" } })
    ));
    assert!(!refused_form(
        &json!({ "kind": "approval", "error": { "kind": "badSignature" } })
    ));
    let unchecked = json!({
        "kind": "genesis",
        "detail": "the proxy code of factory 0x4e1D… on chain 5042002 could not be read"
    });
    assert_eq!(refusal_kind(&unchecked), RefusalKind::SetupUnchecked);
    assert_eq!(
        refusal_kind(&json!({ "kind": "genesis", "detail": "the genesis derives 0x1 on chain 1" })),
        RefusalKind::Setup
    );
    assert_eq!(
        refusal_kind(&json!({ "kind": "approval", "error": { "kind": "badSignature" } })),
        RefusalKind::Signature
    );
    assert_eq!(
        refusal_kind(&json!({ "kind": "tooManyProposals", "limit": 50 })),
        RefusalKind::Full
    );
    assert_eq!(
        refusal_kind(&json!({ "kind": "transaction", "detail": "delegatecall" })),
        RefusalKind::Transaction
    );
    assert_eq!(
        refusal_kind(&json!({ "kind": "error", "status": 404, "error": "unknown Safe" })),
        RefusalKind::Other
    );
}

#[test]
fn a_refusal_is_kept_and_sent_again_with_nothing_signed_twice() {
    let mut w = deployed_world();
    refuse_with(
        &mut w,
        vec![(422, json!({ "refusal": { "kind": "full" } }))],
    );
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(matches!(
        sut.view().step,
        OutsideStep::Failed {
            refused: Some(RefusalKind::Full),
            unsent: true,
            ..
        }
    ));
    assert!(sut.view().unsent);
    assert!(w.store[outside::KEPT_KEY].contains(&addr(SEED_B)));
    let signed = w.submits[0]["approval"]["signature"].clone();
    run(&mut sut, &mut w, page::Event::SendAgain);
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::TypedData
        }
    );
    let posted = mailbox_signatures(&w, &hash);
    assert!(posted
        .iter()
        .any(|(o, s)| *o == addr(SEED_B) && json!(s) == signed));
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 1);
    assert!(!w.store.contains_key(outside::KEPT_KEY));
}

#[test]
fn a_rejection_refused_is_kept_too_and_a_reload_sends_it_again_on_its_own() {
    let mut w = deployed_world();
    refuse_with(&mut w, vec![(404, json!({ "error": "not now" }))]);
    let t = tx("0x3");
    let mut first = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut first,
        &mut w,
        page::Event::Reject {
            account: addr(SEED_B),
        },
    );
    assert!(matches!(
        first.view().step,
        OutsideStep::Failed {
            what: OutsideAction::Reject,
            unsent: true,
            ..
        }
    ));
    // The page reloaded: the same link, a new page.
    let again = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(again.view().step, OutsideStep::Rejected);
    let cancel = w
        .mailbox
        .iter()
        .find(|p| {
            p["safeTx"]["to"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case(SAFE)
        })
        .unwrap();
    assert_eq!(cancel["safeTx"]["nonce"], json!("0x3"));
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 1);
}

#[test]
fn refused_for_the_multisigs_setup_alone_it_is_sent_once_more_without_it() {
    let (mut w, safe) = undeployed_world();
    let once = Rc::new(RefCell::new(true));
    w.submit_hooks.push(Box::new(move |req| {
        let first = std::mem::replace(&mut *once.borrow_mut(), false);
        (first && req.get("genesis").is_some()).then(|| {
            (
                422,
                json!({ "refusal": { "kind": "genesis", "detail": "the proxy code of factory 0x4e1D… on chain 100 could not be read" } }),
                None,
            )
        })
    }));
    let t = tx("0x0");
    let mut sut = opened(&mut w, link_to(&safe, &t));
    assert!(sut.view().tx.unwrap().genesis.is_some());
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    let carried: Vec<bool> = w
        .submits
        .iter()
        .map(|s| s.get("genesis").is_some())
        .collect();
    assert_eq!(carried, vec![true, false]);
    assert_eq!(
        sut.view().step,
        OutsideStep::Approved {
            form: ApprovalForm::TypedData
        }
    );
}

#[test]
fn kept_for_a_number_used_since_it_is_dropped_with_nothing_to_send() {
    let mut w = deployed_world();
    refuse_with(
        &mut w,
        vec![(422, json!({ "refusal": { "kind": "full" } }))],
    );
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    assert!(w.store.contains_key(outside::KEPT_KEY));
    w.safe_mut(CHAIN, SAFE).nonce = 4;
    let used = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(used.view().tx.unwrap().item.status, ItemStatus::Done);
    assert!(!w.store.contains_key(outside::KEPT_KEY));
    assert_eq!(used.view().step, OutsideStep::Idle);
    assert!(!mailbox_signatures(&w, &hash)
        .iter()
        .any(|(o, _)| *o == addr(SEED_B)));
    // r8-24: nothing of its own ran: another transaction took its number.
    assert_eq!(
        used.view().tx.unwrap().done,
        Some(page::OutsideDone::Cancelled)
    );
}

/// r8-24: a link to a transaction that was executed says so, and by which
/// transaction — never only "executed or cancelled".
#[test]
fn a_link_to_an_executed_transaction_says_it_ran_and_where() {
    let mut w = deployed_world();
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    let sut = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(sut.view().tx.unwrap().done, None, "open: nothing to say");
    w.safe_mut(CHAIN, SAFE).nonce = 4;
    let ran = "0x00000000000000000000000000000000000000000000000000000000000000e1";
    w.receipts.insert(
        ran.to_owned(),
        json!({ "logs": [{
            "address": SAFE,
            "topics": ["0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e", hash],
            "data": "0x"
        }] }),
    );
    let used = opened(&mut w, link_to(SAFE, &t));
    assert_eq!(
        used.view().tx.unwrap().done,
        Some(page::OutsideDone::Executed {
            tx_hash: ran.to_owned()
        })
    );
}

// ---------------------------------------------------------------------------
// The connect page's live sessions (`bridge.ts`)
// ---------------------------------------------------------------------------

const MEMBER_SEED: u8 = 8;

fn connect_request(id: &str) -> Value {
    json!({
        "id": id, "method": "walletpair_connect",
        "params": [{ "name": "Vela", "url": ORIGIN, "namespaces": ["eip155"] }],
        "origin": ORIGIN,
    })
}

fn typed_request(id: &str, account: &str, t: &SafeTx) -> Value {
    let typed = multisig::safe_tx_typed_data(CHAIN, SAFE, t).unwrap();
    json!({ "id": id, "method": "eth_signTypedData_v4", "params": [account, typed.to_string()], "origin": ORIGIN })
}

fn started_page(w: &mut World) -> Page {
    let mut sut = Page::new();
    run(
        &mut sut,
        w,
        page::Event::Start {
            service_url: SERVICE.to_owned(),
            wallet_name: "MetaMask".to_owned(),
        },
    );
    sut
}

/// A page joined from Vela's invite for `members`, its connection approved.
fn joined(w: &mut World, members: Vec<String>) -> (Page, String, String) {
    let mut sut = started_page(w);
    run(
        &mut sut,
        w,
        page::Event::Join {
            invite: "wss://relay#invite".into(),
            members,
        },
    );
    let room = format!("room-{}", w.peer.next);
    let session = sut.view().sessions.last().unwrap().id.clone();
    w.peer.deliver(&room, connect_request("c1"));
    settle_ready(&mut sut, w, 20_000);
    assert!(sut.view().connect_prompt.is_some());
    run(
        &mut sut,
        w,
        page::Event::AnswerConnect {
            session: session.clone(),
            approve: true,
        },
    );
    (sut, room, session)
}

fn answer_of(w: &World, room: &str, id: &str) -> Option<Value> {
    w.peer
        .sent_on(room)
        .into_iter()
        .rev()
        .find(|m| m["id"] == json!(id))
}

#[test]
fn a_session_is_approved_on_the_page_and_vela_is_told_its_state() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let (sut, room, _) = joined(&mut w, vec![addr(MEMBER_SEED)]);
    let sent = w.peer.sent_on(&room);
    let state = sent
        .iter()
        .rev()
        .find(|m| m["event"] == json!("state"))
        .unwrap();
    assert_eq!(
        state["data"]["eip155"],
        json!({ "chain": "eip155:100", "accounts": [addr(MEMBER_SEED)] })
    );
    let result = answer_of(&w, &room, "c1").unwrap();
    assert_eq!(result["result"]["wallet"]["name"], json!("MetaMask"));
    assert!(sut.view().sessions[0].approved);
}

#[test]
fn rooms_a_dapp_gave_up_are_ended_once_it_pairs_again_and_the_others_kept() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let session = |id: &str, record: &str, name: &str, origin: &str| {
        json!({
            "id": id, "record": record, "created": 1,
            "dapp": { "name": name, "origin": origin },
            "store": { "approved": ["eip155"], "chains": {}, "records": [], "forwarded": [] },
        })
    };
    w.store.insert(
        outside::SESSIONS_KEY.to_owned(),
        json!([
            session("old-1", "r-old-1", "Vela", ORIGIN),
            session("old-2", "r-old-2", "Vela", ORIGIN),
            session("live", "r-live", "Vela", ORIGIN),
            session("other", "r-other", "Other dApp", "https://other.example"),
        ])
        .to_string(),
    );
    let mut sut = started_page(&mut w);
    run(&mut sut, &mut w, page::Event::ResumeAll);
    assert_eq!(sut.view().sessions.len(), 4);
    // The same dApp, connected right now on another device: kept.
    let live_room = w
        .peer
        .opens
        .iter()
        .position(|(_, p)| p.as_deref() == Some("r-live"))
        .map(|i| format!("room-{}", i + 1))
        .unwrap();
    run(
        &mut sut,
        &mut w,
        page::Event::PeerState {
            room: live_room.clone(),
            state: "connected".into(),
        },
    );
    run(
        &mut sut,
        &mut w,
        page::Event::Join {
            invite: "wss://relay#invite".into(),
            members: vec![],
        },
    );
    let room = format!("room-{}", w.peer.next);
    let fresh = sut.view().sessions.last().unwrap().id.clone();
    w.peer.deliver(&room, connect_request("c1"));
    settle_ready(&mut sut, &mut w, 20_000);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerConnect {
            session: fresh.clone(),
            approve: true,
        },
    );
    let mut ids: Vec<String> = sut.view().sessions.iter().map(|s| s.id.clone()).collect();
    ids.sort();
    let mut want = vec!["live".to_owned(), "other".to_owned(), fresh];
    want.sort();
    assert_eq!(ids, want);
    let ended: Vec<&String> = w
        .peer
        .closed
        .iter()
        .filter(|(_, e)| *e)
        .map(|(r, _)| r)
        .collect();
    assert_eq!(ended.len(), 2, "the two given up");
    assert!(!ended.contains(&&live_room));
}

#[test]
fn every_session_is_kept_when_the_new_one_is_declined() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    w.store.insert(
        outside::SESSIONS_KEY.to_owned(),
        json!([{ "id": "old-1", "record": "r-old-1", "created": 1, "dapp": { "name": "Vela", "origin": ORIGIN }, "store": {} }])
            .to_string(),
    );
    let mut sut = started_page(&mut w);
    run(&mut sut, &mut w, page::Event::ResumeAll);
    run(
        &mut sut,
        &mut w,
        page::Event::Join {
            invite: "wss://relay#invite".into(),
            members: vec![],
        },
    );
    let room = format!("room-{}", w.peer.next);
    let fresh = sut.view().sessions.last().unwrap().id.clone();
    w.peer.deliver(&room, connect_request("c1"));
    settle_ready(&mut sut, &mut w, 20_000);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerConnect {
            session: fresh,
            approve: false,
        },
    );
    assert_eq!(
        answer_of(&w, &room, "c1").unwrap()["error"]["code"],
        json!(4001)
    );
    let view = sut.view();
    assert_eq!(view.sessions.len(), 2);
    assert!(view.sessions[1].declined, "said as declined, not waiting");
}

#[test]
fn a_damaged_invite_is_said_as_the_links_and_nothing_is_saved_or_started() {
    let mut w = World::default();
    w.store.insert(
        outside::SESSIONS_KEY.to_owned(),
        json!([{ "id": "old-1", "record": "r-old-1", "created": 1, "store": {} }]).to_string(),
    );
    let mut sut = started_page(&mut w);
    let before = w.store[outside::SESSIONS_KEY].clone();
    run(
        &mut sut,
        &mut w,
        page::Event::Join {
            invite: "wss://relay#DAMAGED".into(),
            members: vec![],
        },
    );
    assert!(sut.view().join_failed.is_some());
    assert_eq!(sut.view().sessions.len(), 1);
    assert_eq!(w.store[outside::SESSIONS_KEY], before);
}

#[test]
fn a_session_for_the_members_its_link_named_signs_as_them_only() {
    let mut w = with_wallet(&[MEMBER_SEED, SEED_STRANGER]);
    let member = addr(MEMBER_SEED);
    let stranger = addr(SEED_STRANGER);
    let (mut sut, room, session) = joined(
        &mut w,
        vec![member.to_ascii_uppercase().replacen("0X", "0x", 1)],
    );
    let t = tx("0x3");
    w.peer.deliver(&room, typed_request("r1", &stranger, &t));
    w.peer.deliver(
        &room,
        json!({ "id": "r2", "method": "eth_sendTransaction", "params": [{ "from": stranger, "to": SAFE }] }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(
        answer_of(&w, &room, "r1").unwrap()["error"]["code"],
        json!(4001)
    );
    assert_eq!(
        answer_of(&w, &room, "r2").unwrap()["error"]["code"],
        json!(4001)
    );
    let rejected = sut
        .view()
        .activity
        .iter()
        .filter(|a| a.outcome == Some(Outcome::Rejected))
        .count();
    assert_eq!(rejected, 2);
    assert!(wallet(&mut w)
        .log
        .iter()
        .all(|(m, _)| m != "eth_signTypedData_v4"));
    // The member: held for the owner's tap, shown first (r4C-7).
    w.peer.deliver(&room, typed_request("r3", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    let shown = sut.view().request.unwrap();
    assert_eq!(shown.id, "r3");
    assert_eq!(shown.stage, Stage::Review);
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session: session.clone(),
            id: "r3".into(),
            go: true,
        },
    );
    assert!(answer_of(&w, &room, "r3").unwrap()["result"].is_string());
    // Asking the wallet to switch networks names no account: it goes.
    w.peer.deliver(
        &room,
        json!({ "id": "r4", "method": "wallet_switchEthereumChain", "params": [{ "chainId": "0x64" }] }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(answer_of(&w, &room, "r4").unwrap()["result"], Value::Null);
}

#[test]
fn an_older_link_named_no_one_the_wallet_is_asked_as_before() {
    let mut w = with_wallet(&[SEED_STRANGER]);
    let (mut sut, room, session) = joined(&mut w, vec![]);
    w.peer
        .deliver(&room, typed_request("r1", &addr(SEED_STRANGER), &tx("0x3")));
    settle_ready(&mut sut, &mut w, 20_000);
    assert!(sut.view().request.is_some());
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    assert!(answer_of(&w, &room, "r1").unwrap()["result"].is_string());
}

#[test]
fn a_request_is_read_and_held_for_the_tap_the_wallet_asked_only_then() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    let t = tx("0x3");
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    let shown = sut.view().request.unwrap();
    let read = shown.safe_tx.unwrap();
    assert_eq!(
        hash_of(&read.safe, &read.tx),
        hash_of(SAFE, &t),
        "the core read the SafeTx"
    );
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    let entry = sut.view().activity[0].clone();
    assert_eq!(entry.account.as_deref(), Some(member.as_str()));
    assert!(entry.wallet_at.is_some());
    assert_eq!(entry.outcome, Some(Outcome::Done));
    assert_eq!(entry.form, Some(EcdsaForm::TypedData));
}

/// r4C-14: Vela's request, counted as the mailbox and the chain hold it
/// now — its approvals already enough (and none of them its account's):
/// nothing to sign, whatever Vela still thinks.
#[test]
fn a_request_whose_approvals_are_already_enough_is_said_so() {
    let member = addr(MEMBER_SEED);
    let t = tx("0x3");
    let hash = hash_of(SAFE, &t);
    for (approvals, enough) in [
        (vec![approval_by(SEED_A, &hash)], false),
        (
            vec![
                approval_by(SEED_A, &hash),
                approval_by(SEED_STRANGER, &hash),
            ],
            true,
        ),
    ] {
        let mut w = with_wallet(&[MEMBER_SEED]);
        let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &member, &addr(SEED_STRANGER)], 2);
        safe.nonce = 3;
        w.deploy(CHAIN, SAFE, safe);
        w.mailbox = vec![stored(SAFE, &t, approvals)];
        let (mut sut, room, _) = joined(&mut w, vec![member.clone()]);
        w.peer.deliver(&room, typed_request("r1", &member, &t));
        settle_ready(&mut sut, &mut w, 20_000);
        assert!(sut.view().request.is_some());
        assert_eq!(sut.view().request_enough, enough);
    }
}

/// r5reg-23: Vela's request names its multisig as the link's review does —
/// from the registry's genesis, only if the core derives the address from
/// it — even before the mailbox holds the transaction.
#[test]
fn a_request_names_its_multisig_from_the_registry() {
    let member = addr(MEMBER_SEED);
    let mut w = with_wallet(&[MEMBER_SEED]);
    let mut g = world::genesis(&[&addr(SEED_A), &member], 2, "0x05");
    g.genesis.name = Some("R5 Team".into());
    w.record(&g.address, &g.genesis, false);
    let t = tx("0x0");
    let (mut sut, room, _) = joined(&mut w, vec![member.clone()]);
    let typed = multisig::safe_tx_typed_data(CHAIN, &g.address, &t).unwrap();
    w.peer.deliver(
        &room,
        json!({ "id": "r1", "method": "eth_signTypedData_v4", "params": [member, typed.to_string()], "origin": ORIGIN }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert!(sut.view().request.is_some());
    let named = sut.view().request_genesis.unwrap();
    assert_eq!(named.name.as_deref(), Some("R5 Team"));
    assert!(
        !sut.view().request_enough,
        "not in the mailbox: Sign is offered"
    );
}

/// The page came up, then its wallet was picked: started again with the
/// wallet's name, the sessions joined meanwhile stand.
#[test]
fn started_again_with_the_wallets_name_its_sessions_stand() {
    let mut w = with_wallet(&[SEED_A]);
    let (mut sut, _, session) = joined(&mut w, Vec::new());
    run(
        &mut sut,
        &mut w,
        page::Event::Start {
            service_url: SERVICE.to_owned(),
            wallet_name: "Rabby".to_owned(),
        },
    );
    assert_eq!(sut.view().sessions.len(), 1);
    assert_eq!(sut.view().sessions[0].id, session);
}

#[test]
fn declined_here_or_withdrawn_by_vela_while_shown_said_as_each() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    let t = tx("0x3");
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: false,
        },
    );
    assert_eq!(
        answer_of(&w, &room, "r1").unwrap()["error"]["code"],
        json!(4001)
    );
    assert_eq!(sut.view().activity[0].stage, Stage::Answered);
    assert_eq!(sut.view().activity[0].outcome, Some(Outcome::Rejected));

    w.peer
        .deliver(&room, typed_request("r2", &member, &tx("0x4")));
    settle_ready(&mut sut, &mut w, 20_000);
    w.peer.deliver(
        &room,
        json!({ "id": "x1", "method": "walletpair_cancel", "params": ["r2"] }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().activity[0].outcome, Some(Outcome::Withdrawn));
    assert!(sut.view().request.is_none());
    assert_eq!(answer_of(&w, &room, "x1").unwrap()["result"], Value::Null);
}

#[test]
fn vela_asking_again_for_the_same_transaction_withdraws_what_it_asked_before() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    let t = tx("0x3");
    w.peer.deliver(&room, typed_request("stale", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    w.peer.deliver(&room, typed_request("again", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(
        answer_of(&w, &room, "stale").unwrap()["error"]["code"],
        json!(4001)
    );
    let stale = sut
        .view()
        .activity
        .into_iter()
        .find(|a| a.id == "stale")
        .unwrap();
    assert_eq!(stale.outcome, Some(Outcome::Withdrawn));
    assert_eq!(
        sut.view().request.unwrap().id,
        "again",
        "never the stale one first"
    );
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "again".into(),
            go: true,
        },
    );
    assert!(answer_of(&w, &room, "again").unwrap()["result"].is_string());
}

#[test]
fn withdrawn_once_in_the_wallet_said_whatever_the_wallet_answers() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    w.peer
        .deliver(&room, typed_request("r1", &member, &tx("0x3")));
    settle_ready(&mut sut, &mut w, 20_000);
    wallet(&mut w).hold = true;
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    w.peer.deliver(
        &room,
        json!({ "id": "x1", "method": "walletpair_cancel", "params": ["r1"] }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().activity[0].outcome, Some(Outcome::Withdrawn));
    wallet(&mut w).hold = false;
    settle_ready(&mut sut, &mut w, 20_000);
    let entry = sut.view().activity[0].clone();
    assert_eq!(entry.stage, Stage::Answered);
    assert_eq!(entry.outcome, Some(Outcome::Withdrawn));
}

#[test]
fn each_request_ends_with_its_outcome_signed_declined_or_failed_in_its_words() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    let mut ask = |w: &mut World, id: &str, nonce: &str, answer: Option<Res>| {
        if let Some(a) = answer {
            wallet(w).next.push_back(("eth_signTypedData_v4".into(), a));
        }
        w.peer
            .deliver(&room, typed_request(id, &member, &tx(nonce)));
        settle_ready(&mut sut, w, 20_000);
        run(
            &mut sut,
            w,
            page::Event::AnswerRequest {
                session: session.clone(),
                id: id.into(),
                go: true,
            },
        );
        sut.view().activity[0].clone()
    };
    let done = ask(&mut w, "r1", "0x3", None);
    assert_eq!(done.outcome, Some(Outcome::Done));
    assert!(done.result.is_some());
    let declined = ask(
        &mut w,
        "r2",
        "0x4",
        Some(Res::WalletRefused {
            code: Some(4001),
            message: "User rejected the request.".into(),
        }),
    );
    assert_eq!(declined.outcome, Some(Outcome::Rejected));
    assert_eq!(declined.problem, Some(WalletProblem::Declined));
    let failed = ask(
        &mut w,
        "r3",
        "0x5",
        Some(Res::WalletRefused {
            code: Some(-32603),
            message: r#"{"message":"Ledger: \"0x6985\" denied"}"#.into(),
        }),
    );
    assert_eq!(failed.outcome, Some(Outcome::Failed));
    assert_eq!(
        failed.problem,
        Some(WalletProblem::Unknown {
            reason: r#"Ledger: "0x6985" denied"#.into()
        })
    );
}

#[test]
fn no_typed_data_in_the_wallet_the_hash_in_the_eth_sign_form_on_a_tap() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    wallet(&mut w).typed = false;
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    let t = tx("0x3");
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session: session.clone(),
            id: "r1".into(),
            go: true,
        },
    );
    assert_eq!(sut.view().request.unwrap().stage, Stage::Fallback);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    let (method, params) = wallet(&mut w).log.last().cloned().unwrap();
    assert_eq!(method, "personal_sign");
    assert_eq!(params, json!([hash_of(SAFE, &t), member]));
    let sig = answer_of(&w, &room, "r1").unwrap()["result"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(sig.ends_with("1f") || sig.ends_with("20"));
    let entry = sut.view().activity[0].clone();
    assert_eq!(entry.outcome, Some(Outcome::Done));
    assert_eq!(entry.form, Some(EcdsaForm::EthSign));
    // Checked as the Safe checks it: the member's, for this transaction.
    assert!(outside::checked(
        &hash_of(SAFE, &t),
        std::slice::from_ref(&member),
        &member,
        &sig
    )
    .is_some());
}

#[test]
fn a_request_on_another_network_than_the_session_saw_is_not_acted_on() {
    let mut w = with_wallet(&[MEMBER_SEED]);
    let member = addr(MEMBER_SEED);
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    w.peer
        .deliver(&room, typed_request("r1", &member, &tx("0x3")));
    settle_ready(&mut sut, &mut w, 20_000);
    // The wallet moved while the owner looked (no event yet).
    wallet(&mut w).chain_id = 1;
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    assert_eq!(
        answer_of(&w, &room, "r1").unwrap()["error"]["code"],
        json!(4901)
    );
    assert_eq!(wallet(&mut w).calls_of("eth_signTypedData_v4"), 0);
}

#[test]
fn the_pages_own_way_onto_the_relay_is_the_worst_of_its_rooms() {
    use relay::{worst, RelayStatus, RelayTrouble};
    let failed = RelayStatus::Failed {
        why: RelayTrouble::Refused,
        detail: "HTTP 4403".into(),
    };
    assert_eq!(worst(&[RelayStatus::Up, RelayStatus::Up]), RelayStatus::Up);
    assert_eq!(
        worst(&[RelayStatus::Up, failed.clone(), RelayStatus::Connecting]),
        failed
    );
}

/// r4C-1: the transport's word on a room's way onto the relay (its token
/// refused, none given) is the page's own, said until it is up again.
#[test]
fn the_transports_word_on_a_rooms_relay_is_the_pages() {
    use relay::{RelayStatus, RelayTrouble};
    let mut w = with_wallet(&[SEED_A]);
    let (mut sut, room, _) = joined(&mut w, Vec::new());
    let refused = RelayStatus::Failed {
        why: RelayTrouble::Refused,
        detail: "HTTP 4403".into(),
    };
    run(
        &mut sut,
        &mut w,
        page::Event::Relay {
            room: room.clone(),
            status: refused.clone(),
        },
    );
    assert_eq!(sut.view().relay, refused);
    assert_eq!(sut.view().sessions[0].relay, refused);
    // Another page's room, or none: nothing changes.
    run(
        &mut sut,
        &mut w,
        page::Event::Relay {
            room: "room-elsewhere".into(),
            status: RelayStatus::Up,
        },
    );
    assert_eq!(sut.view().relay, refused);
    run(
        &mut sut,
        &mut w,
        page::Event::PeerState {
            room,
            state: "connected".into(),
        },
    );
    assert_eq!(sut.view().relay, RelayStatus::Up);
}

/// The wallet refusing to connect the page is said in its words, and gone
/// once it connects.
#[test]
fn a_wallet_that_will_not_connect_the_page_is_said() {
    let mut w = deployed_world();
    wallet(&mut w).next.push_back((
        "eth_requestAccounts".into(),
        Res::WalletRefused {
            code: Some(4001),
            message: "User rejected".into(),
        },
    ));
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    run(&mut sut, &mut w, page::Event::ConnectWallet);
    assert_eq!(sut.view().wallet_failure, Some(WalletProblem::Declined));
    assert!(sut.view().accounts.is_empty());
    run(&mut sut, &mut w, page::Event::ConnectWallet);
    assert_eq!(sut.view().wallet_failure, None);
    assert_eq!(sut.view().accounts, vec![addr(SEED_B).to_ascii_lowercase()]);
}

// ---------------------------------------------------------------------------
// Vela's side of a live session (`session.svelte.ts`, `ExternalSignSheet`)
// ---------------------------------------------------------------------------

fn request() -> SessionRequest {
    let t = tx("0x3");
    SessionRequest {
        chain_id: CHAIN,
        safe: SAFE.into(),
        safe_tx_hash: hash_of(SAFE, &t),
        tx: t,
        owners: vec![addr(SEED_B)],
        send: None,
    }
}

fn start_session(w: &mut World) -> Session {
    let mut sut = Session::new();
    run(
        &mut sut,
        w,
        session::Event::Start {
            request: Box::new(request()),
            service_url: SERVICE.to_owned(),
            relay: None,
            origin: Some(ORIGIN.to_owned()),
        },
    );
    sut
}

fn paired_before(w: &mut World) {
    w.store.insert(
        outside::PAIRINGS_KEY.to_owned(),
        json!([{ "id": "p1", "record": "record-1", "accounts": [addr(SEED_B)], "wallet": "Rabby", "savedMs": 1 }])
            .to_string(),
    );
}

fn saved_ids(w: &World) -> Vec<String> {
    serde_json::from_str::<Vec<Value>>(
        w.store
            .get(outside::PAIRINGS_KEY)
            .map_or("[]", String::as_str),
    )
    .unwrap()
    .iter()
    .map(|r| r["id"].as_str().unwrap().to_owned())
    .collect()
}

fn last_sent(w: &World, room: &str, method: &str) -> Value {
    w.peer
        .sent_on(room)
        .into_iter()
        .rev()
        .find(|m| m["method"] == json!(method))
        .unwrap_or_else(|| panic!("no {method} sent"))
}

/// The page's side, scripted: it connects with `accounts` on `chain`.
fn page_connects(w: &mut World, sut: &mut Session, room: &str, accounts: &[String], chain: u64) {
    let connect = last_sent(w, room, "walletpair_connect");
    let state = json!({ "eip155": { "chain": format!("eip155:{chain}"), "accounts": accounts } });
    w.peer
        .deliver(room, json!({ "event": "state", "data": state }));
    w.peer.deliver(
        room,
        json!({ "id": connect["id"], "result": { "wallet": { "name": "Rabby" }, "state": state } }),
    );
    settle_ready(sut, w, 20_000);
}

/// The page answers Vela's signature request with the wallet's own answer.
fn page_signs(w: &mut World, sut: &mut Session, room: &str) {
    let ask = last_sent(w, room, "eth_signTypedData_v4");
    let answer = wallet(w).answer("eth_signTypedData_v4", &ask["params"]);
    let Res::WalletAnswered { result } = answer else {
        panic!("{answer:?}")
    };
    w.peer
        .deliver(room, json!({ "id": ask["id"], "result": result }));
    settle_ready(sut, w, 20_000);
}

#[test]
fn a_paired_wallet_is_the_one_asked_again_and_stays_paired_after_leaving() {
    let mut w = World::default();
    paired_before(&mut w);
    let mut sut = start_session(&mut w);
    let view = sut.view();
    assert!(view.resumed, "the page only needs reopening: no new code");
    assert_eq!(view.invite, None);
    assert_eq!(view.paired_account.as_deref(), Some(addr(SEED_B).as_str()));
    assert_eq!(view.wallet.as_deref(), Some("Rabby"));
    assert_eq!(w.peer.opens[0], (None, Some("record-1".to_owned())));
    run(&mut sut, &mut w, session::Event::Close);
    assert_eq!(
        w.peer.closed,
        vec![("room-1".to_owned(), false)],
        "left, not ended"
    );
    assert_eq!(saved_ids(&w), vec!["p1".to_owned()]);
    assert!(
        start_session(&mut w).view().resumed,
        "the next signature finds it again"
    );
}

#[test]
fn a_pairing_is_forgotten_once_ended_from_either_side() {
    let mut w = World::default();
    paired_before(&mut w);
    let mut sut = start_session(&mut w);
    w.peer.close("room-1", "closed-by-peer");
    settle_ready(&mut sut, &mut w, 20_000);
    assert!(saved_ids(&w).is_empty());

    paired_before(&mut w);
    let mut sut = start_session(&mut w);
    run(
        &mut sut,
        &mut w,
        session::Event::AnotherWallet { unpair: true },
    );
    assert!(saved_ids(&w).is_empty(), "Unpair forgets it");
    assert!(
        w.peer.closed.iter().any(|(_, end)| *end),
        "ended for both sides"
    );
    assert!(!sut.view().resumed, "the next request is a new room");
    assert!(start_session(&mut w).view().invite.is_some());
}

#[test]
fn a_new_room_has_its_invite_and_the_members_for_the_link() {
    let mut w = World::default();
    let sut = start_session(&mut w);
    let view = sut.view();
    assert!(!view.resumed);
    assert_eq!(view.invite.as_deref(), Some("wss://relay#invite-1"));
    assert_eq!(view.members, vec![addr(SEED_B)]);
    let tx = view.tx.unwrap();
    assert_eq!(tx.safe_tx_hash, request().safe_tx_hash);
}

#[test]
fn use_another_wallet_is_a_fresh_room_and_the_saved_pairing_is_kept() {
    let mut w = World::default();
    paired_before(&mut w);
    let mut sut = start_session(&mut w);
    assert!(sut.view().resumed);
    run(
        &mut sut,
        &mut w,
        session::Event::AnotherWallet { unpair: false },
    );
    let view = sut.view();
    assert!(!view.resumed);
    assert!(view.invite.is_some(), "a fresh code");
    assert_eq!(w.peer.closed, vec![("room-1".to_owned(), false)]);
    assert_eq!(saved_ids(&w), vec!["p1".to_owned()]);
}

#[test]
fn this_sides_relay_state_is_followed() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    assert_eq!(sut.view().relay, relay::RelayStatus::Connecting);
    let failed = relay::RelayStatus::Failed {
        why: relay::RelayTrouble::Refused,
        detail: "HTTP 4403".into(),
    };
    run(
        &mut sut,
        &mut w,
        session::Event::Relay {
            status: failed.clone(),
        },
    );
    assert_eq!(sut.view().relay, failed);
}

#[test]
fn connected_signed_and_verified_before_it_is_used() {
    let mut w = with_wallet(&[SEED_B]);
    let mut sut = start_session(&mut w);
    assert_eq!(sut.view().phase, Phase::Waiting);
    let started = sut.view().since;
    w.now += 5_000.0;
    run(
        &mut sut,
        &mut w,
        session::Event::PeerState {
            room: "room-1".into(),
            state: "connected".into(),
        },
    );
    assert_eq!(sut.view().phase, Phase::Connecting);
    assert!(
        sut.view().since > started,
        "timed from when the phase began"
    );
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    assert_eq!(sut.view().phase, Phase::Confirming);
    assert_eq!(sut.view().wallet.as_deref(), Some("Rabby"));
    page_signs(&mut w, &mut sut, "room-1");
    let view = sut.view();
    let Some(SessionOutcome::Approved { approval }) = view.outcome else {
        panic!("{:?}", view.outcome)
    };
    assert!(multisig::verify_ecdsa_approval(&request().safe_tx_hash, &approval).is_ok());
    assert_eq!(view.phase, Phase::Done);
    assert_eq!(
        w.peer.closed,
        vec![("room-1".to_owned(), false)],
        "left: the pairing stays"
    );
    assert_eq!(saved_ids(&w).len(), 1, "paired: kept for next time");
}

#[test]
fn an_answer_that_is_not_the_owners_is_said_and_never_used() {
    let mut w = with_wallet(&[SEED_B, SEED_STRANGER]);
    wallet(&mut w).signer = Some(addr(SEED_STRANGER));
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    page_signs(&mut w, &mut sut, "room-1");
    assert_eq!(sut.view().failure, Some(Failure::BadSignature));
    assert!(sut.view().outcome.is_none());
}

#[test]
fn an_account_that_may_not_sign_is_said_until_the_wallet_switches() {
    let mut w = with_wallet(&[SEED_B]);
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_STRANGER)], CHAIN);
    assert_eq!(sut.view().phase, Phase::WrongAccount);
    assert!(w
        .peer
        .sent_on("room-1")
        .iter()
        .all(|m| m["method"] != json!("eth_signTypedData_v4")));
    let state = json!({ "eip155": { "chain": "eip155:100", "accounts": [addr(SEED_B)] } });
    w.peer
        .deliver("room-1", json!({ "event": "state", "data": state }));
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().phase, Phase::Confirming);
}

#[test]
fn the_wallet_is_asked_to_switch_networks_and_a_stay_is_said() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], 1);
    assert_eq!(sut.view().phase, Phase::Switching);
    let switch = last_sent(&w, "room-1", "wallet_switchEthereumChain");
    assert_eq!(switch["params"], json!([{ "chainId": "0x64" }]));
    // The page answers, but the wallet stayed where it was.
    w.peer
        .deliver("room-1", json!({ "id": switch["id"], "result": null }));
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().failure, Some(Failure::OtherChain));
}

#[test]
fn a_connection_declined_on_the_page_is_said_as_that() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    let connect = last_sent(&w, "room-1", "walletpair_connect");
    w.peer.deliver(
        "room-1",
        json!({ "id": connect["id"], "error": { "code": 4001, "message": "User rejected the request" } }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().failure, Some(Failure::ConnectDeclined));
}

#[test]
fn a_wallet_silent_for_a_while_is_timed_said_and_can_be_stopped() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    let view = sut.view();
    assert_eq!(view.phase, Phase::Confirming);
    assert!(!view.stalled(view.since + 5_000.0));
    assert!(view.stalled(view.since + session::STILL_WAITING_MS + 1.0));
    let ask = last_sent(&w, "room-1", "eth_signTypedData_v4");
    run(&mut sut, &mut w, session::Event::StopWaiting);
    assert_eq!(sut.view().failure, Some(Failure::StoppedWaiting));
    let cancel = last_sent(&w, "room-1", "walletpair_cancel");
    assert_eq!(cancel["params"], json!([ask["id"]]), "the page is told");
}

#[test]
fn ask_again_withdraws_the_ask_and_makes_it_again_and_a_late_valid_answer_is_welcome() {
    let mut w = with_wallet(&[SEED_B]);
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    let first = last_sent(&w, "room-1", "eth_signTypedData_v4");
    run(&mut sut, &mut w, session::Event::AskAgain);
    let second = last_sent(&w, "room-1", "eth_signTypedData_v4");
    assert_ne!(first["id"], second["id"]);
    assert_eq!(
        last_sent(&w, "room-1", "walletpair_cancel")["params"],
        json!([first["id"]])
    );
    // The first ask's answer arrives after all: a valid approval from any run.
    let answer = wallet(&mut w).answer("eth_signTypedData_v4", &first["params"]);
    let Res::WalletAnswered { result } = answer else {
        panic!()
    };
    w.peer
        .deliver("room-1", json!({ "id": first["id"], "result": result }));
    settle_ready(&mut sut, &mut w, 20_000);
    assert!(matches!(
        sut.view().outcome,
        Some(SessionOutcome::Approved { .. })
    ));
}

#[test]
fn a_wallet_failure_in_its_own_words() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    let ask = last_sent(&w, "room-1", "eth_signTypedData_v4");
    w.peer.deliver(
        "room-1",
        json!({ "id": ask["id"], "error": { "code": -32603, "message": "Ledger: denied" } }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(
        sut.view().failure,
        Some(Failure::Wallet {
            problem: WalletProblem::Unknown {
                reason: "Ledger: denied".into()
            }
        })
    );
}

/// r5reg-11: "Use another wallet", then the old room's answer fails late:
/// nothing is said for a room no longer asked.
#[test]
fn a_late_answer_from_a_room_left_behind_changes_nothing() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    let ask = last_sent(&w, "room-1", "eth_signTypedData_v4");
    run(
        &mut sut,
        &mut w,
        session::Event::AnotherWallet { unpair: false },
    );
    w.peer.deliver(
        "room-1",
        json!({ "id": ask["id"], "error": { "code": 4900, "message": "Disconnected" } }),
    );
    settle_ready(&mut sut, &mut w, 20_000);
    assert_eq!(sut.view().failure, None);
    assert_eq!(sut.view().phase, Phase::Waiting);
}

#[test]
fn an_approval_that_reaches_the_mailbox_from_the_link_alone_ends_the_request() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    let r = request();
    w.mailbox = vec![stored(
        SAFE,
        &r.tx,
        vec![approval_by(SEED_B, &r.safe_tx_hash)],
    )];
    run(&mut sut, &mut w, session::Event::MailboxTick);
    let Some(SessionOutcome::Approved { approval }) = sut.view().outcome else {
        panic!("{:?}", sut.view().outcome)
    };
    assert!(approval.owner.eq_ignore_ascii_case(&addr(SEED_B)));
    assert_eq!(
        w.peer.closed,
        vec![("room-1".to_owned(), false)],
        "the room left"
    );
}

#[test]
fn the_page_going_away_mid_request_is_said_until_it_is_back() {
    let mut w = World::default();
    let mut sut = start_session(&mut w);
    run(
        &mut sut,
        &mut w,
        session::Event::PeerState {
            room: "room-1".into(),
            state: "connected".into(),
        },
    );
    page_connects(&mut w, &mut sut, "room-1", &[addr(SEED_B)], CHAIN);
    run(
        &mut sut,
        &mut w,
        session::Event::PeerState {
            room: "room-1".into(),
            state: "offline".into(),
        },
    );
    assert!(sut.view().away);
    let before = w.peer.sent_on("room-1").len();
    run(
        &mut sut,
        &mut w,
        session::Event::PeerState {
            room: "room-1".into(),
            state: "connected".into(),
        },
    );
    assert!(!sut.view().away);
    // Back after a break: the pending ask is sent again (core §6).
    let resent = &w.peer.sent_on("room-1")[before..];
    assert!(resent
        .iter()
        .any(|m| m["method"] == json!("eth_signTypedData_v4")));
}

// ---------------------------------------------------------------------------
// Both sides of the channel, connected
// ---------------------------------------------------------------------------

/// Vela opens a room; the page joins from its invite; the owner approves the
/// connection, reads the request on the page, taps; the wallet signs; Vela
/// checks the answer and has its approval — the pairing kept on both sides.
#[test]
fn end_to_end_vela_asks_the_page_shows_it_the_owner_taps_vela_verifies() {
    let mut w = with_wallet(&[SEED_B]);
    let mut vela = start_session(&mut w);
    let invite = vela.view().invite.unwrap();
    let mut pg = started_page(&mut w);
    run(
        &mut pg,
        &mut w,
        page::Event::Join {
            invite,
            members: vela.view().members,
        },
    );
    let session_id = pg.view().sessions[0].id.clone();
    let (vela_room, page_room) = ("room-1".to_owned(), "room-2".to_owned());
    let mut forwarded = 0;
    let mut pump = |w: &mut World, vela: &mut Session, pg: &mut Page| {
        for _ in 0..20 {
            settle_ready(vela, w, 20_000);
            settle_ready(pg, w, 20_000);
            let fresh: Vec<(String, Value)> = w.peer.sent[forwarded..].to_vec();
            forwarded = w.peer.sent.len();
            if fresh.is_empty() {
                break;
            }
            for (room, message) in fresh {
                let to = if room == vela_room {
                    &page_room
                } else {
                    &vela_room
                };
                w.peer.deliver(to, message);
            }
        }
    };
    pump(&mut w, &mut vela, &mut pg);
    assert_eq!(
        pg.view().connect_prompt.unwrap().dapp.name,
        "Vela",
        "the page asks the owner first"
    );
    pg.dispatch(page::Event::AnswerConnect {
        session: session_id.clone(),
        approve: true,
    });
    pump(&mut w, &mut vela, &mut pg);
    assert_eq!(vela.view().phase, Phase::Confirming);
    let shown = pg.view().request.unwrap();
    assert_eq!(
        shown.safe_tx.as_ref().map(|r| hash_of(&r.safe, &r.tx)),
        Some(request().safe_tx_hash)
    );
    pg.dispatch(page::Event::AnswerRequest {
        session: session_id,
        id: shown.id,
        go: true,
    });
    pump(&mut w, &mut vela, &mut pg);
    let Some(SessionOutcome::Approved { approval }) = vela.view().outcome else {
        panic!("{:?}", vela.view())
    };
    assert!(approval.owner.eq_ignore_ascii_case(&addr(SEED_B)));
    assert!(multisig::verify_ecdsa_approval(&request().safe_tx_hash, &approval).is_ok());
    assert_eq!(saved_ids(&w).len(), 1, "Vela keeps the pairing");
    assert_eq!(pg.view().sessions.len(), 1, "the page keeps its session");
}

// ---------------------------------------------------------------------------
// The SafeTx a typed-data request asks for, and Safe's eth_sign form (`typed`)
// ---------------------------------------------------------------------------

#[test]
fn a_typed_data_request_reads_as_a_safe_tx_only_when_it_is_the_cores_own() {
    let t = tx("0x3");
    let typed = multisig::safe_tx_typed_data(5_042_002, SAFE, &t).unwrap();
    let read = outside::read_safe_tx_request(&json!(typed.to_string())).unwrap();
    assert_eq!(read.chain_id, 5_042_002);
    assert_eq!(
        multisig::safe_tx_hash(read.chain_id, &read.safe, &read.tx).unwrap(),
        multisig::safe_tx_hash(5_042_002, SAFE, &t).unwrap()
    );
    // An object, as some sides send it, reads the same.
    assert_eq!(outside::read_safe_tx_request(&typed), Some(read));
}

#[test]
fn anything_else_is_not_a_safe_tx_request() {
    let typed = multisig::safe_tx_typed_data(5_042_002, SAFE, &tx("0x3")).unwrap();
    let mut changed = typed.clone();
    changed["types"]["SafeTx"].as_array_mut().unwrap().pop();
    assert!(outside::read_safe_tx_request(&changed).is_none());
    let mut named = typed.clone();
    named["domain"]["name"] = json!("Not Safe");
    assert!(outside::read_safe_tx_request(&named).is_none());
    let mut other = typed.clone();
    other["primaryType"] = json!("Permit");
    assert!(outside::read_safe_tx_request(&other).is_none());
    let mut moved = typed;
    moved["message"]["to"] = json!(SAFE);
    assert!(
        outside::read_safe_tx_request(&moved).is_some(),
        "itself, re-read"
    );
    assert!(outside::read_safe_tx_request(&json!("not json")).is_none());
    assert!(outside::read_safe_tx_request(&Value::Null).is_none());
}

#[test]
fn safes_eth_sign_form_is_v_plus_4_over_the_eip191_hash() {
    let hash = multisig::safe_tx_hash(5_042_002, SAFE, &tx("0x3")).unwrap();
    let hashes = multisig::safe_tx_hashes(5_042_002, SAFE, &tx("0x3")).unwrap();
    let (key, owner) = eoa(SEED_A);
    let personal = eoa_sign(
        &key,
        &vela_core::primitives::from_hex(&hashes.eth_sign_hash).unwrap(),
    );
    // The bare personal_sign answer is no approval of the safeTxHash.
    assert!(multisig::verify_ecdsa_approval(
        &hash,
        &EcdsaApproval {
            owner: owner.clone(),
            signature: personal.clone()
        }
    )
    .is_err());
    let form = outside::eth_sign_form(&personal).unwrap();
    assert!(form.ends_with("1f") || form.ends_with("20"));
    let checked = multisig::verify_ecdsa_approval(
        &hash,
        &EcdsaApproval {
            owner: owner.clone(),
            signature: form.clone(),
        },
    )
    .unwrap();
    assert!(checked.owner.eq_ignore_ascii_case(&owner));
    assert!(checked.signature.eq_ignore_ascii_case(&form));
    assert_eq!(multisig::ecdsa_form(&form).unwrap(), EcdsaForm::EthSign);
    let typed = sign_hash(SEED_A, &hash);
    assert_eq!(multisig::ecdsa_form(&typed).unwrap(), EcdsaForm::TypedData);
}

#[test]
fn eth_sign_form_takes_v_27_28_or_0_1_and_nothing_else() {
    let rs = "11".repeat(64);
    assert_eq!(
        outside::eth_sign_form(&format!("0x{rs}1b")).unwrap(),
        format!("0x{rs}1f")
    );
    assert_eq!(
        outside::eth_sign_form(&format!("0x{rs}01")).unwrap(),
        format!("0x{rs}20")
    );
    assert!(outside::eth_sign_form(&format!("0x{rs}1f")).is_err());
    assert!(outside::eth_sign_form("0x1234").is_err());
}

#[test]
fn domain_and_message_hash_make_the_safe_tx_hash() {
    let h = multisig::safe_tx_hashes(5_042_002, SAFE, &tx("0x3")).unwrap();
    let mut joined = vec![0x19, 0x01];
    joined.extend(vela_core::primitives::from_hex(&h.domain_hash).unwrap());
    joined.extend(vela_core::primitives::from_hex(&h.message_hash).unwrap());
    assert_eq!(
        vela_core::primitives::to_hex(&vela_core::primitives::keccak256(&joined), true),
        h.safe_tx_hash
    );
}

#[test]
fn approvals_on_chain_calldata() {
    let a = "0x00000000000000000000000000000000000000aa";
    let hash = format!("0x{}", "ab".repeat(32));
    assert_eq!(
        outside::approved_hashes_call(a, &hash),
        format!("0x7d832974{}aa{}", "0".repeat(62), "ab".repeat(32))
    );
    assert_eq!(
        outside::approve_hash_call(&hash),
        format!("0xd4d9bdcd{}", "ab".repeat(32))
    );
}

/// JSON vectors: each plain-account approval, as the Safe will read it.
#[test]
fn the_outside_vectors_replay() {
    let path = format!(
        "{}/tests/vectors/multisig/outside.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for case in v["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let tx: SafeTx = serde_json::from_value(case["tx"].clone()).unwrap();
        let chain = case["chainId"].as_u64().unwrap();
        let safe = case["safe"].as_str().unwrap();
        let hash = multisig::safe_tx_hash(chain, safe, &tx).unwrap();
        assert_eq!(json!(hash), case["safeTxHash"], "{name}");
        let owners: Vec<String> = vec![case["owner"].as_str().unwrap().to_owned()];
        for s in case["signatures"].as_array().unwrap() {
            let sig = s["signature"].as_str().unwrap();
            let got = outside::checked(&hash, &owners, &owners[0], sig);
            assert_eq!(
                got.is_some(),
                s["accepted"].as_bool().unwrap(),
                "{name}: {}",
                s["about"]
            );
            let form = multisig::ecdsa_form(sig).ok().map(|f| json!(f));
            assert_eq!(
                form.unwrap_or(Value::Null),
                s["form"],
                "{name}: {}",
                s["about"]
            );
        }
        if let Some(personal) = case["personal"].as_str() {
            assert_eq!(
                json!(outside::eth_sign_form(personal).unwrap()),
                case["ethSignForm"],
                "{name}"
            );
        }
    }
}

/// Re-captures `outside.json` (`VELA_WRITE_VECTORS=1 cargo test … -- --ignored`).
#[test]
#[ignore = "writes the vectors"]
fn write_the_outside_vectors() {
    let mut cases = Vec::new();
    for (name, chain, nonce) in [
        ("transfer at 3", 5_042_002u64, "0x3"),
        ("transfer at 0, Gnosis", 100, "0x0"),
    ] {
        let t = tx(nonce);
        let hash = multisig::safe_tx_hash(chain, SAFE, &t).unwrap();
        let hashes = multisig::safe_tx_hashes(chain, SAFE, &t).unwrap();
        let (key, owner) = eoa(SEED_A);
        let (other, _) = eoa(SEED_STRANGER);
        let typed = eoa_sign(&key, &vela_core::primitives::from_hex(&hash).unwrap());
        let personal = eoa_sign(
            &key,
            &vela_core::primitives::from_hex(&hashes.eth_sign_hash).unwrap(),
        );
        let form = outside::eth_sign_form(&personal).unwrap();
        let forged = eoa_sign(&other, &vela_core::primitives::from_hex(&hash).unwrap());
        cases.push(json!({
            "name": name, "chainId": chain, "safe": SAFE, "tx": t, "safeTxHash": hash, "owner": owner,
            "personal": personal, "ethSignForm": form,
            "signatures": [
                { "about": "EIP-712, v 27/28", "signature": typed, "accepted": true, "form": "typedData" },
                { "about": "Safe's eth_sign form, v 31/32", "signature": form, "accepted": true, "form": "ethSign" },
                { "about": "the bare personal_sign answer", "signature": personal, "accepted": false, "form": "typedData" },
                { "about": "another key's", "signature": forged, "accepted": false, "form": "typedData" },
                { "about": "too short", "signature": "0x1234", "accepted": false, "form": null }
            ]
        }));
    }
    let out = json!({
        "about": "Plain-account (EOA) approvals of a SafeTx as the outside-owner machines check them: the owner key is k256 seed [7; 32]; another key is [9; 32].",
        "cases": cases,
    });
    if std::env::var("VELA_WRITE_VECTORS").is_ok() {
        let path = format!(
            "{}/tests/vectors/multisig/outside.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(path, serde_json::to_string_pretty(&out).unwrap() + "\n").unwrap();
    }
}

// ---------------------------------------------------------------------------
// A wallet's refusal, said (`errors`)
// ---------------------------------------------------------------------------

#[test]
fn a_wallet_error_by_its_eip1193_code() {
    use outside::wallet_problem as p;
    assert_eq!(
        p(Some(4001), "User rejected the request."),
        WalletProblem::Declined
    );
    assert_eq!(p(Some(4100), "Unauthorized"), WalletProblem::Unauthorized);
    // Some wallets say it without the code.
    assert_eq!(p(None, "Unauthorized"), WalletProblem::Unauthorized);
    assert_eq!(
        p(None, "user denied transaction signature"),
        WalletProblem::Declined
    );
    assert_eq!(p(Some(4200), "x"), WalletProblem::Unsupported);
    assert_eq!(
        p(
            Some(-32601),
            "the method eth_signTypedData_v4 does not exist"
        ),
        WalletProblem::Unsupported
    );
    assert_eq!(p(Some(4901), "x"), WalletProblem::OtherChain);
    assert_eq!(p(Some(4900), "x"), WalletProblem::Disconnected);
}

#[test]
fn anything_else_in_the_wallets_own_words_cut_to_one_readable_line() {
    assert_eq!(
        outside::wallet_problem(
            Some(-32603),
            r#"{"message":"Ledger: \"0x6985\" denied by the user"}"#
        ),
        WalletProblem::Unknown {
            reason: r#"Ledger: "0x6985" denied by the user"#.into()
        }
    );
    assert_eq!(
        outside::short_reason("Error: Error: something\nstack line 1\nstack line 2"),
        "something"
    );
    assert_eq!(outside::short_reason(&"x".repeat(400)).chars().count(), 120);
    assert_eq!(
        outside::wallet_problem(Some(-32000), ""),
        WalletProblem::Unknown {
            reason: "-32000".into()
        }
    );
}

// ---------------------------------------------------------------------------
// Who an outside wallet may sign for (`signers`)
// ---------------------------------------------------------------------------

/// The one rule of what a member is and how it signs, replayed from
/// `tests/vectors/multisig/signers.json` (signers.test.ts, member-kind.test.ts,
/// and the owner's signer-configuration rule).
#[test]
fn the_signer_vectors_hold() {
    use vela_core::app::multisig_signers::{kind_choice, kind_evidence, KindEvidence, MemberKind};
    let path = format!(
        "{}/tests/vectors/multisig/signers.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let facts = |v: &Value| -> SignerFacts { serde_json::from_value(v.clone()).unwrap() };
    let kinds = v["signerKind"].as_array().unwrap();
    assert!(kinds.len() >= 20);
    for case in kinds {
        let got = outside::signer_kind(&facts(&case["facts"]));
        assert_eq!(json!(got), case["expect"], "{}", case["name"]);
    }
    let evidence = v["evidence"].as_array().unwrap();
    assert!(evidence.len() >= 10);
    for case in evidence {
        let got = kind_evidence(&facts(&case["facts"]));
        assert_eq!(json!(got), case["expect"], "{}", case["name"]);
    }
    let choices = v["kindChoice"].as_array().unwrap();
    assert!(choices.len() >= 11);
    for case in choices {
        let e: Option<KindEvidence> = serde_json::from_value(case["evidence"].clone()).unwrap();
        let said: Option<MemberKind> = serde_json::from_value(case["said"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(kind_choice(e, said)).unwrap(),
            case["expect"],
            "{}",
            case["name"]
        );
    }
    for case in v["plainAccounts"].as_array().unwrap() {
        let owners: Vec<(String, SignerFacts)> = case["owners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["address"].as_str().unwrap().to_owned(),
                    facts(&o["facts"]),
                )
            })
            .collect();
        let names: Vec<String> = owners.iter().map(|(a, _)| a.clone()).collect();
        let plain = outside::plain_accounts(&names, |a| {
            owners.iter().find(|(o, _)| o == a).unwrap().1.clone()
        });
        assert_eq!(json!(plain), case["expect"], "{}", case["name"]);
    }
    // A Vela member on this device is never anything but `here`.
    assert_eq!(
        outside::signer_kind(&SignerFacts {
            local: true,
            code: Some(true),
            configured: Some(false),
            ..SignerFacts::default()
        }),
        Some(SignerKind::Here)
    );
}

#[test]
fn only_a_counted_approval_is_evidence() {
    use outside::{approved_with, ApprovalRow, ApprovedWith};
    let g = vela_core::dev_fixtures::account(0).unwrap();
    let keys = vec![vela_core::user_op::WalletKey {
        credential_id: g.credential_id_hex.clone(),
        public_key_hex: g.public_key_hex.clone(),
    }];
    let (_, owner_genesis) = multisig::vela_account_genesis(&keys).unwrap();
    let carol = "0x00000000000000000000000000000000000000c1";
    let passkey = |counts: bool| ApprovalRow {
        owner: carol.into(),
        counts,
        approval: multisig::Approval::Passkey(Box::new(multisig::PasskeyApproval {
            owner: carol.into(),
            owner_genesis: owner_genesis.clone(),
            signer: "0x".into(),
            authenticator_data: "0x".into(),
            client_data_fields: String::new(),
            r: "0x1".into(),
            s: "0x1".into(),
        })),
        created_ms: 0.0,
    };
    assert_eq!(approved_with(&[vec![passkey(false)]], carol), None);
    assert_eq!(
        approved_with(&[vec![passkey(true)]], carol),
        Some(ApprovedWith::Passkey)
    );
    let wallet_row = ApprovalRow {
        owner: carol.into(),
        counts: true,
        approval: multisig::Approval::Ecdsa(EcdsaApproval {
            owner: carol.into(),
            signature: "0x".into(),
        }),
        created_ms: 0.0,
    };
    assert_eq!(
        approved_with(&[vec![wallet_row]], carol),
        Some(ApprovedWith::Wallet)
    );
}

// ---------------------------------------------------------------------------
// A Vela member's approval on its trusted page (`page-approval`)
// ---------------------------------------------------------------------------

#[test]
fn a_page_approval_is_accepted_only_as_the_chain_will_and_asked_once() {
    let fixture = vela_core::dev_fixtures::account(0).unwrap();
    let keys = vec![vela_core::user_op::WalletKey {
        credential_id: fixture.credential_id_hex.clone(),
        public_key_hex: fixture.public_key_hex.clone(),
    }];
    let (member, _) = multisig::vela_account_genesis(&keys).unwrap();
    let hash = hash_of(SAFE, &tx("0x3"));
    let answer = |challenge: &str| {
        let bytes = vela_core::primitives::from_hex(challenge).unwrap();
        let a = vela_core::dev_fixtures::build_assertion(
            &fixture,
            &bytes,
            vela_core::dev_fixtures::RP_ID,
            vela_core::dev_fixtures::ORIGIN,
        )
        .unwrap();
        let approval = multisig::approval_from_assertion(
            &keys,
            &a.credential_id_hex,
            &a.authenticator_data_hex,
            &a.client_data_json_hex,
            &a.signature_der_hex,
        )
        .unwrap();
        serde_json::to_value(approval).unwrap()
    };
    let challenge = multisig::approval_challenge(CHAIN, &member, &hash).unwrap();
    let good = outside::accept_page_approval(&answer(&challenge), CHAIN, &member, &hash).unwrap();
    assert!(multisig::verify_approval(CHAIN, &hash, &good).is_ok());
    // The bare safeTxHash: what a page that skipped the member's SafeMessage would sign.
    assert!(outside::accept_page_approval(&answer(&hash), CHAIN, &member, &hash).is_err());
    // Another member's word for it.
    assert!(outside::accept_page_approval(
        &answer(&challenge),
        CHAIN,
        "0x00000000000000000000000000000000000000aa",
        &hash
    )
    .is_err());
    let mut answers = outside::PageAnswers::default();
    assert!(answers.get(&member, CHAIN, &hash).is_none());
    answers.keep(CHAIN, &hash, good.clone());
    assert_eq!(
        answers.get(
            &member.to_ascii_lowercase(),
            CHAIN,
            &hash.to_ascii_uppercase().replacen("0X", "0x", 1)
        ),
        Some(&good)
    );
}

// ---------------------------------------------------------------------------
// The relay token's rules (`relay`)
// ---------------------------------------------------------------------------

#[test]
fn only_the_public_relay_asks_for_a_token() {
    assert!(relay::needs_token(relay::DEFAULT_RELAY));
    assert!(relay::needs_token(&format!("{}/", relay::DEFAULT_RELAY)));
    assert!(!relay::needs_token("ws://127.0.0.1:8792"));
}

#[test]
fn a_kept_token_is_used_until_it_is_about_to_expire() {
    use relay::{usable, KeptToken, TokenSource};
    let now = 1_800_000_000.0;
    let year = KeptToken {
        token: "t-site".into(),
        expires_at: now + 365.0 * 24.0 * 3600.0,
        source: Some(TokenSource::Site),
    };
    assert_eq!(
        usable(Some(&year), now, false),
        Some(("t-site".into(), TokenSource::Site))
    );
    // The trial's is asked for: the site's does not do.
    assert_eq!(usable(Some(&year), now, true), None);
    let soon = KeptToken {
        expires_at: now + 60.0,
        ..year.clone()
    };
    assert_eq!(
        usable(Some(&soon), now, false),
        None,
        "replaced before it is used"
    );
    assert_eq!(usable(None, now, false), None);
    // Forgotten only when it is the refused one.
    assert!(!relay::forgets(Some(&year), Some("t-other")));
    assert!(relay::forgets(Some(&year), Some("t-site")));
    assert!(relay::forgets(Some(&year), None));
}

#[test]
fn what_a_token_endpoint_answered_and_what_is_said_when_none_did() {
    let token = relay::token_answer(
        Some(200),
        r#"{"token":"t-1","expires_at":1900000000,"relay_url":"wss://relay.peerconnect.dev"}"#,
    )
    .unwrap();
    assert_eq!(token.token, "t-1");
    assert_eq!(
        relay::token_answer(Some(503), r#"{"error":"not_configured"}"#),
        Err("HTTP 503".into())
    );
    assert_eq!(
        relay::token_answer(Some(200), "{}"),
        Err("no token in the answer".into())
    );
    assert_eq!(relay::token_answer(None, ""), Err("no answer".into()));
    assert_eq!(
        relay::no_token_detail(relay::TOKEN_ENDPOINT, Some("HTTP 429"), "HTTP 429"),
        "getvela.app: HTTP 429 · peerconnect.dev: HTTP 429"
    );
}

/// r4C-1: a refused token is never followed by another at once: 2 s, then
/// 4 s, a fresh one from the site, then the trial's once, and stopped.
#[test]
fn a_connection_refused_by_the_relay_waits_falls_back_to_the_trial_once_then_stops() {
    use relay::{Before, Refusals, TokenSource, BACKOFF_MS};
    let mut r = Refusals::default();
    assert_eq!(r.before_token(), Before::Go { trial: false });
    r.issued(TokenSource::Site);
    r.unauthorized();
    assert_eq!(r.before_token(), Before::Wait { ms: BACKOFF_MS[0] });
    assert_eq!(r.before_token(), Before::Go { trial: false });
    r.issued(TokenSource::Site);
    r.unauthorized();
    assert_eq!(r.before_token(), Before::Wait { ms: BACKOFF_MS[1] });
    assert_eq!(
        r.before_token(),
        Before::Go { trial: true },
        "the trial's, once"
    );
    r.issued(TokenSource::Trial);
    r.unauthorized();
    assert_eq!(r.before_token(), Before::Stop, "no loop: said as failed");
    // Retry: asked again, from the site.
    r.retried();
    assert_eq!(r.before_token(), Before::Go { trial: false });
}

#[test]
fn getting_no_token_backs_off_to_30_s_then_stops() {
    use relay::{no_token_step, Before, BACKOFF_MS};
    let waits: Vec<Before> = (0..=BACKOFF_MS.len()).map(no_token_step).collect();
    assert_eq!(waits[0], Before::Wait { ms: 2_000 });
    assert_eq!(waits[4], Before::Wait { ms: 30_000 });
    assert_eq!(waits[5], Before::Stop);
}

#[test]
fn up_once_on_the_relay_and_unreachable_when_a_token_does_not_get_it_there() {
    use relay::{on_state, OnState, RelayStatus, RelayTrouble, UNREACHABLE_AFTER_MS};
    let s = on_state(
        &RelayStatus::Connecting,
        "offline",
        UNREACHABLE_AFTER_MS + 10.0,
        relay::DEFAULT_RELAY,
    );
    assert_eq!(
        s,
        OnState {
            status: RelayStatus::Retrying {
                why: RelayTrouble::Unreachable,
                detail: "relay.peerconnect.dev".into(),
                at: None
            },
            accepted: false
        }
    );
    // core-ms-2: on the relay is accepted (its refusals forgotten), in Rust.
    assert_eq!(
        on_state(&s.status, "waiting", 0.0, relay::DEFAULT_RELAY),
        OnState {
            status: RelayStatus::Up,
            accepted: true
        }
    );
    assert!(on_state(&RelayStatus::Up, "connected", 0.0, relay::DEFAULT_RELAY).accepted);
    assert_eq!(
        on_state(&RelayStatus::Up, "offline", 0.0, relay::DEFAULT_RELAY),
        OnState {
            status: RelayStatus::Connecting,
            accepted: false
        }
    );
}

// ---------------------------------------------------------------------------
// WalletPair's messages
// ---------------------------------------------------------------------------

#[test]
fn the_account_a_request_signs_or_sends_as() {
    use outside::signing_account as of;
    let a = "0xAbCdEf0000000000000000000000000000000001";
    let lower = a.to_ascii_lowercase();
    assert_eq!(
        of("eth_sendTransaction", &json!([{ "from": a }])),
        Some(lower.clone())
    );
    assert_eq!(
        of("eth_signTypedData_v4", &json!([a, "{}"])),
        Some(lower.clone())
    );
    assert_eq!(of("personal_sign", &json!(["0x00", a])), Some(lower));
    assert_eq!(
        of("wallet_switchEthereumChain", &json!([{ "chainId": "0x1" }])),
        None
    );
    assert!(
        outside::expected(&[], a),
        "any account, when the link named none"
    );
    assert!(outside::expected(&[a.to_ascii_lowercase()], a));
    assert!(!outside::expected(&["0x01".into()], a));
}

#[test]
fn a_dapp_side_reads_responses_events_and_ignores_the_rest() {
    use outside::{parse_from_wallet as read, parse_state, FromWallet};
    assert_eq!(
        read(&json!({ "id": "1", "result": "0xsig" })),
        FromWallet::Response {
            id: "1".into(),
            answer: Ok(json!("0xsig"))
        }
    );
    assert_eq!(
        read(&json!({ "id": "1", "error": { "code": 4001, "message": "no" } })),
        FromWallet::Response {
            id: "1".into(),
            answer: Err((4001, "no".into()))
        }
    );
    assert_eq!(
        read(&json!({ "id": "1", "result": 1, "error": {} })),
        FromWallet::Ignore
    );
    assert_eq!(read(&json!({ "id": 7, "result": 1 })), FromWallet::Ignore);
    assert!(matches!(
        read(&json!({ "event": "state", "data": {} })),
        FromWallet::Event { .. }
    ));
    let account = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
    let state = parse_state(
        &json!({ "eip155": { "chain": "eip155:100", "accounts": [account] }, "solana": {} }),
    );
    assert_eq!(state.unwrap().unwrap().accounts, vec![account.to_owned()]);
    assert_eq!(parse_state(&json!({})), Some(None));
    assert_eq!(
        parse_state(&json!({ "eip155": { "chain": "eip155:0100", "accounts": [account] } })),
        None
    );
    assert_eq!(
        parse_state(
            &json!({ "eip155": { "chain": "eip155:1", "accounts": [account.to_ascii_uppercase()] } })
        ),
        None
    );
}

// ---------------------------------------------------------------------------
// r4C-12 / r7-11: what each row was, and the request reviewed as the link is
// ---------------------------------------------------------------------------

/// `addOwnerWithThreshold(owner, threshold)` from the multisig to itself.
fn add_owner(owner: &str, threshold: u8, nonce: &str) -> SafeTx {
    SafeTx {
        to: SAFE.into(),
        value: "0x0".into(),
        data: format!(
            "0x0d582f13{:0>64}{:0>64x}",
            owner.trim_start_matches("0x").to_ascii_lowercase(),
            threshold
        ),
        ..tx(nonce)
    }
}

/// r8-11: a paired request whose review could not be gathered (the chain did
/// not answer) is reviewed again on the page's next refresh — never left on
/// its title and hashes alone, without the warnings the link's review says.
#[test]
fn a_request_reviewed_while_the_chain_is_away_is_reviewed_again() {
    let member = addr(MEMBER_SEED);
    let mut w = with_wallet(&[MEMBER_SEED]);
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &member, &addr(SEED_STRANGER)], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let t = add_owner(&addr(43), 2, "0x3");
    let (mut sut, room, _session) = joined(&mut w, vec![member.clone()]);
    w.silent.insert(CHAIN);
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    let first = sut.view().request_review;
    assert!(
        first.as_ref().is_none_or(|s| s.review.is_none()),
        "nothing could be read: {first:?}"
    );
    w.silent.remove(&CHAIN);
    run(&mut sut, &mut w, page::Event::Refresh);
    let shown = sut.view().request_review.expect("the request is reviewed");
    assert!(shown.review.is_some(), "the review is gathered again");
}

/// Vela asks to approve adding a member: its row says what it is, and its
/// review is the core one, drawn as the link one is, the member change
/// rule with it (r7-11: Approvals 2 of 3, then 2 of 4) even before the
/// mailbox holds it.
#[test]
fn a_request_is_said_by_what_it_is_and_reviewed_with_its_member_change() {
    use outside::reading::{RowLabel, RowOutcome, TxTitle};
    let member = addr(MEMBER_SEED);
    let mut w = with_wallet(&[MEMBER_SEED]);
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &member, &addr(SEED_STRANGER)], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let newcomer = addr(42);
    let t = add_owner(&newcomer, 2, "0x3");
    let (mut sut, room, session) = joined(&mut w, vec![member.clone()]);
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);

    let shown = sut.view().request_review.expect("the request is reviewed");
    assert_eq!(shown.nonce, "3");
    assert_eq!(shown.chain_nonce.as_deref(), Some("3"));
    assert_eq!(
        shown.hashes.safe_tx_hash.to_ascii_lowercase(),
        hash_of(SAFE, &t).to_ascii_lowercase()
    );
    match &shown.title {
        TxTitle::AddOwner { owner } => assert!(owner.eq_ignore_ascii_case(&newcomer)),
        other => panic!("not an addition: {other:?}"),
    }
    let review = shown.review.expect("the core reviewed it");
    let changes = &review["memberChanges"];
    assert_eq!(changes["before"], json!({ "threshold": 2, "owners": 3 }));
    assert_eq!(changes["after"], json!({ "threshold": 2, "owners": 4 }));
    assert_eq!(changes["fits"], json!(true));

    let row = sut.view().rows[0].clone();
    assert_eq!(row.id, "r1");
    match &row.label {
        RowLabel::Approve { nonce, title, .. } => {
            assert_eq!(nonce, "3");
            assert!(matches!(title, TxTitle::AddOwner { .. }));
        }
        other => panic!("not an approval: {other:?}"),
    }
    assert_eq!(row.outcome, RowOutcome::Waiting);
    run(
        &mut sut,
        &mut w,
        page::Event::AnswerRequest {
            session,
            id: "r1".into(),
            go: true,
        },
    );
    let row = sut.view().rows[0].clone();
    assert_eq!(row.outcome, RowOutcome::Done);
    assert!(!row.bad);
}

/// A token the registry does not name is read once, by the core: its
/// symbol and decimals are the contract own word, said as such.
#[test]
fn a_request_sending_a_token_carries_its_facts_read_by_the_core() {
    use outside::reading::{RowLabel, TxTitle};
    let member = addr(MEMBER_SEED);
    let mut w = with_wallet(&[MEMBER_SEED]);
    let mut safe = DeployedSafe::clean(&[&addr(SEED_A), &member], 2);
    safe.nonce = 3;
    w.deploy(CHAIN, SAFE, safe);
    let token = "0x1111111111111111111111111111111111111111";
    w.rpc_hooks.push(Box::new(move |_, method, params| {
        if method != "eth_call" {
            return None;
        }
        let to = params[0]["to"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if to != token {
            return None;
        }
        let data = params[0]["data"].as_str().unwrap_or_default();
        let word = |n: u64| format!("{n:064x}");
        let answer = match data {
            "0x95d89b41" => format!("0x{}{}{:0<64}", word(32), word(3), "544b4e"),
            "0x313ce567" => format!("0x{}", word(6)),
            _ => return None,
        };
        Some(Res::RpcResult {
            result: json!(answer),
        })
    }));
    let to = "0x000000000000000000000000000000000000bEEF";
    let t = SafeTx {
        to: token.into(),
        value: "0x0".into(),
        data: format!(
            "0xa9059cbb{:0>64}{:0>64x}",
            to.trim_start_matches("0x").to_ascii_lowercase(),
            2_500_000u64
        ),
        ..tx("0x3")
    };
    let (mut sut, room, _) = joined(&mut w, vec![member.clone()]);
    w.peer.deliver(&room, typed_request("r1", &member, &t));
    settle_ready(&mut sut, &mut w, 20_000);
    let shown = sut.view().request_review.unwrap();
    match &shown.title {
        TxTitle::Send {
            amount,
            token: facts,
            to: recipient,
        } => {
            assert_eq!(amount, "2500000");
            assert!(recipient.eq_ignore_ascii_case(to));
            assert_eq!(facts.token.as_deref(), Some(token));
            assert_eq!(facts.symbol.as_deref(), Some("TKN"));
            assert_eq!(facts.decimals, Some(6));
            assert!(!facts.known, "the contract own word, not the registry");
        }
        other => panic!("not a transfer: {other:?}"),
    }
    match &sut.view().rows[0].label {
        RowLabel::ApproveTo { to: who, title, .. } => {
            assert!(who.eq_ignore_ascii_case(to));
            assert!(
                matches!(title, TxTitle::Send { token, .. } if token.symbol.as_deref() == Some("TKN"))
            );
        }
        other => panic!("not an approval to someone: {other:?}"),
    }
}

/// What was done here on the link transaction is a row too, with how it
/// ended: approved, then rejected.
#[test]
fn what_was_done_here_on_the_links_transaction_is_a_row() {
    use outside::reading::{RowLabel, RowOutcome, TxTitle};
    let mut w = deployed_world();
    let t = tx("0x3");
    let mut sut = opened(&mut w, link_to(SAFE, &t));
    assert!(sut.view().rows.is_empty());
    match &sut.view().tx.unwrap().title {
        TxTitle::Send { amount, token, .. } => {
            assert_eq!(amount, "10000000000000000");
            assert_eq!(token.token, None, "the network coin");
        }
        other => panic!("not a transfer: {other:?}"),
    }
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].id.starts_with("here-"));
    match &rows[0].label {
        RowLabel::ApproveTo { nonce, to, .. } => {
            assert_eq!(nonce, "3");
            assert!(to.eq_ignore_ascii_case(&t.to));
        }
        other => panic!("not an approval: {other:?}"),
    }
    assert_eq!(rows[0].outcome, RowOutcome::Done);
    // Read again: the same step is not logged twice.
    run(&mut sut, &mut w, page::Event::Refresh);
    assert_eq!(sut.view().rows.len(), 1);
}

/// `execTransaction` carries no number: the SafeTx it runs is matched,
/// field by field and byte by byte, to one this page has seen.
#[test]
fn an_exec_transaction_is_matched_to_the_safe_tx_it_runs() {
    use outside::reading::executed_nonce;
    let t = tx("0x7");
    let data = multisig::exec_transaction_data(&t, "0x").unwrap();
    let known = vec![(SAFE.to_owned(), t.clone())];
    assert_eq!(executed_nonce(SAFE, &data, &known).as_deref(), Some("7"));
    assert_eq!(
        executed_nonce(
            &SAFE.to_ascii_lowercase(),
            &data.to_ascii_uppercase().replacen("0X", "0x", 1),
            &known
        )
        .as_deref(),
        Some("7"),
        "any case"
    );
    // Another multisig, or another transaction: not this one.
    assert_eq!(executed_nonce(MODULE, &data, &known), None);
    let other = SafeTx {
        value: "0x1".into(),
        ..t.clone()
    };
    assert_eq!(
        executed_nonce(
            SAFE,
            &multisig::exec_transaction_data(&other, "0x").unwrap(),
            &known
        ),
        None
    );
    let bytes = SafeTx {
        data: "0xdeadbeef".into(),
        ..t.clone()
    };
    assert_eq!(
        executed_nonce(
            SAFE,
            &multisig::exec_transaction_data(&bytes, "0x").unwrap(),
            &known
        ),
        None,
        "its bytes are matched too"
    );
    assert_eq!(executed_nonce(SAFE, "0xa9059cbb", &known), None);
}
