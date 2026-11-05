//! Proposing — and approving — through an outside member's wallet when no
//! member is on this device: the web's `proposeBuilt` → `approveExternally`
//! (`NoOutsideSignerError`, `plainAccounts`) and `ExternalSignSheet`'s
//! answer rule. The proposals machine hands the reviewed draft (or an open
//! proposal) to the outside session ([`Notice::OutsideSign`]) after the
//! signing gate; the session's answer is verified here — the owner's for
//! this safeTxHash, one of those asked — and submitted through the outbox.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::json;
use support::DomainDriver;
use vela_core::app::multisig_dapp::HeldStage;
use vela_core::app::multisig_outside::session::SessionOutcome;
use vela_core::app::multisig_proposals::{ActionState, Event, MultisigProposals, ProposalKind};
use vela_core::app::multisig_shell::{MultisigAccount, Notice};
use vela_core::multisig::{self, EcdsaApproval, Transfer};
use world::{eoa, eoa_sign, fixture, genesis, settle_reads_first, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    // A shell's clock: a bound (the review's) runs out after the reads it bounds.
    let answered = settle_reads_first(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// A 2-of-3 of three plain accounts (seeds 1, 2, 3); this device holds a
/// Vela account that is none of them.
fn setup() -> (World, Sut, MultisigAccount) {
    let owners: Vec<String> = [1u8, 2, 3].iter().map(|s| eoa(*s).1).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = genesis(&refs, 2, "0x61");
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
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    (world, sut, account)
}

fn draft(sut: &mut Sut, world: &mut World) {
    run(
        sut,
        world,
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
    assert!(sut.view().draft.is_some());
}

/// `seed`'s wallet signs `hash` (its owner is `owner`).
fn signed(seed: u8, owner: &str, hash: &str) -> EcdsaApproval {
    let digest = vela_core::primitives::from_hex(hash).unwrap();
    EcdsaApproval {
        owner: owner.to_owned(),
        signature: eoa_sign(&eoa(seed).0, &digest),
    }
}

fn asked(sut: &Sut) -> (String, Vec<String>) {
    match sut.view().actions["propose"].clone() {
        ActionState::OutsideAsked {
            safe_tx_hash,
            owners,
        } => (safe_tx_hash, owners),
        other => panic!("{other:?}"),
    }
}

fn answered(hash: &str, outcome: SessionOutcome) -> Event {
    Event::OutsideAnswered {
        chain_id: 100,
        safe_tx_hash: hash.to_owned(),
        outcome,
    }
}

/// The reviewed draft goes to an outside member's wallet: the gate first,
/// then the session is started with every plain-account owner; its
/// verified answer is submitted through the outbox — one approval, the
/// draft done.
#[test]
fn no_member_here_proposes_through_an_outside_wallet() {
    let (mut world, mut sut, account) = setup();
    draft(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, owners) = asked(&sut);
    assert_eq!(owners, account.owners, "every plain account may sign");
    assert!(world.passkeys.is_empty(), "no passkey here");
    let request = world
        .notices
        .iter()
        .find_map(|n| match n {
            Notice::OutsideSign { request } => Some(request.clone()),
            _ => None,
        })
        .expect("the session is started");
    assert_eq!(request.safe_tx_hash, hash);
    assert_eq!(request.safe, account.address);
    assert!(request.send.is_none());
    // r12: its genesis goes with it — an outside page can review a multisig
    // no service records and no chain holds yet (it derives the address).
    assert_eq!(request.genesis.as_ref(), Some(&account.genesis));
    assert!(sut.view().draft.is_some(), "the draft stays while asked");
    assert!(world.submits.is_empty());

    let approval = signed(2, &owners[1], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval }),
    );
    assert!(matches!(
        sut.view().actions["propose"],
        ActionState::Done { .. }
    ));
    assert!(sut.view().draft.is_none());
    assert_eq!(world.submits.len(), 1);
    let item = sut.view().items[0].clone();
    assert_eq!(item.safe_tx_hash, hash);
    assert_eq!(item.approved, 1);
    assert!(item.approvals[0].counts);
}

/// An answer that is not an asked owner's for this hash is never submitted.
#[test]
fn an_answer_that_does_not_check_out_is_not_submitted() {
    let (mut world, mut sut, _) = setup();
    draft(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, owners) = asked(&sut);
    // Signed by another key in the owner's name.
    let forged = signed(9, &owners[0], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval: forged }),
    );
    assert!(matches!(
        sut.view().actions["propose"],
        ActionState::Failed { .. }
    ));
    assert!(world.submits.is_empty());
    // A second answer for the same ask is not taken either.
    let late = signed(1, &owners[0], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval: late }),
    );
    assert!(world.submits.is_empty());
}

/// Someone who is not a member, signing for themselves, is no approval.
#[test]
fn a_stranger_is_not_one_of_those_asked() {
    let (mut world, mut sut, _) = setup();
    draft(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, _) = asked(&sut);
    let stranger = eoa(9).1;
    let approval = signed(9, &stranger, &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval }),
    );
    assert!(matches!(
        sut.view().actions["propose"],
        ActionState::Failed { .. }
    ));
    assert!(world.submits.is_empty());
}

/// The sheet closed: cancelled, the draft still there; withdrawn while the
/// wallet was asked: what it signed goes nowhere.
#[test]
fn a_closed_session_or_a_withdrawn_draft_sends_nothing() {
    let (mut world, mut sut, _) = setup();
    draft(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, owners) = asked(&sut);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Cancelled),
    );
    assert_eq!(sut.view().actions["propose"], ActionState::Cancelled);
    assert!(sut.view().draft.is_some());

    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, _) = asked(&sut);
    run(&mut sut, &mut world, Event::Withdraw);
    let approval = signed(1, &owners[0], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval }),
    );
    assert_eq!(sut.view().actions["propose"], ActionState::Cancelled);
    assert!(world.submits.is_empty());
}

/// r4C-10: every member is a Vela account (the passkey index knows them):
/// none can sign through an outside wallet — nothing is proposed.
#[test]
fn no_outside_signer_proposes_nothing() {
    let (mut world, mut sut, account) = setup();
    run(
        &mut sut,
        &mut world,
        Event::IndexedMembers {
            addresses: account.owners.clone(),
        },
    );
    draft(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(sut.view().actions["propose"], ActionState::NoOutsideSigner);
    assert!(!world
        .notices
        .iter()
        .any(|n| matches!(n, Notice::OutsideSign { .. })));
}

/// An open proposal approved from an outside wallet: those who approved
/// already are not asked again.
#[test]
fn an_open_proposal_is_approved_from_an_outside_wallet() {
    let (mut world, mut sut, account) = setup();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let first = signed(1, &account.owners[0], &hash);
    world.mailbox.push(json!({
        "chainId": 100, "safe": account.address, "safeTxHash": hash, "safeTx": tx,
        "proposer": first.owner, "approvals": [{"owner": first.owner, "signature": first.signature, "createdMs": 1}],
        "createdMs": 1, "updatedMs": 1,
    }));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert_eq!(sut.view().items[0].approved, 1);
    run(
        &mut sut,
        &mut world,
        Event::ApproveOutside {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
        },
    );
    let key = format!("approve:{hash}");
    let owners = match sut.view().actions[&key].clone() {
        ActionState::OutsideAsked { owners, .. } => owners,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        owners,
        account.owners[1..].to_vec(),
        "not the one who approved"
    );
    let approval = signed(3, &account.owners[2], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval }),
    );
    assert!(matches!(sut.view().actions[&key], ActionState::Done { .. }));
    assert_eq!(sut.view().items[0].approved, 2);
}

/// A dApp's transaction proposed through an outside wallet is held as any.
#[test]
fn a_dapps_transaction_proposed_through_an_outside_wallet_is_held() {
    let (mut world, mut sut, account) = setup();
    run(
        &mut sut,
        &mut world,
        Event::DappRequest {
            id: "r".into(),
            account: Box::new(account.clone()),
            method: "eth_sendTransaction".into(),
            params: json!([{ "to": "0x000000000000000000000000000000000000dEaD", "value": "0x1" }]),
            origin: "https://app.example".into(),
            chain_id: 100,
            limit_ms: None,
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let (hash, owners) = asked(&sut);
    assert_eq!(sut.view().dapp["r"].stage, HeldStage::Reviewing);
    let approval = signed(1, &owners[0], &hash);
    run(
        &mut sut,
        &mut world,
        answered(&hash, SessionOutcome::Approved { approval }),
    );
    let held = sut.view().dapp["r"].clone();
    assert_eq!(held.stage, HeldStage::Waiting);
    assert_eq!(held.safe_tx_hash.as_deref(), Some(hash.as_str()));
    assert_eq!(held.answer, None);
}
