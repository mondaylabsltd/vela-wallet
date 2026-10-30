//! One review, one gate, for every signer of a multisig transaction: the
//! connect page (an outside owner's wallet, `multisig_outside::page`) reads
//! and decides exactly as the proposals machine does for a Vela member
//! (`multisig_proposals`) — both through `multisig_gather`. The pure rule is
//! pinned by `tests/vectors/multisig/gate.json`; here the same world (the
//! same chain, the same mailbox, the same simulation and recipient facts) is
//! put to both machines, and they must see the same review and come to the
//! same gate.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_gather::decide_with;
use vela_core::app::multisig_outside::page::{self, OutsidePage, PageGate};
use vela_core::app::multisig_outside::TxLink;
use vela_core::app::multisig_proposals::{self as proposals, ActionState, MultisigProposals};
use vela_core::app::multisig_shell::{MultisigAccount, MultisigShellResult as Res};
use vela_core::multisig::{self, SafeTx};
use vela_core::multisig_review::SignRule;
use world::{
    eoa, eoa_sign, fixture, settle_ready, DeployedSafe, WalletSim, World, FACTORY, SERVICE,
};

const CHAIN: u64 = 100;
const MODULE: &str = "0x9813461E88aDD60D0eC0943B74A83368CA2E36D1";
const RECIPIENT: &str = "0x000000000000000000000000000000000000dEaD";
const SEED_A: u8 = 7;
const SEED_B: u8 = 8;

// ---------------------------------------------------------------------------
// The rule
// ---------------------------------------------------------------------------

#[test]
fn the_gate_vectors_hold() {
    let path = format!(
        "{}/tests/vectors/multisig/gate.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 12);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let findings: Option<Vec<Value>> =
            serde_json::from_value(case["findings"].clone()).unwrap();
        let sign: Option<SignRule> = serde_json::from_value(case["sign"].clone()).unwrap();
        let gate = decide_with(
            case["chainId"].as_u64().unwrap(),
            findings,
            sign,
            case["decodable"].as_bool().unwrap(),
        );
        assert_eq!(
            serde_json::to_value(&gate).unwrap(),
            case["expect"],
            "{name}"
        );
    }
}

// ---------------------------------------------------------------------------
// The same world, put to both machines
// ---------------------------------------------------------------------------

/// What the world is like when each machine is asked to sign.
#[derive(Clone, Copy, Debug)]
enum Scene {
    /// Nothing besides its members can act for it; a plain transfer.
    Clean,
    /// A module is enabled on it: something besides the members can act.
    Module,
    /// A delegatecall into an unknown contract (Bybit's shape).
    Delegatecall,
    /// The chain does not answer who can act for it.
    Silent,
    /// Not deployed, and its genesis's factory is missing here.
    RefusedGenesis,
}

fn addr(seed: u8) -> String {
    eoa(seed).1
}

fn tx(scene: Scene, nonce: &str) -> SafeTx {
    let mut t = SafeTx {
        to: RECIPIENT.into(),
        value: "0x2386f26fc10000".into(),
        data: "0x".into(),
        operation: 0,
        safe_tx_gas: "0x0".into(),
        base_gas: "0x0".into(),
        gas_price: "0x0".into(),
        gas_token: "0x0000000000000000000000000000000000000000".into(),
        refund_receiver: "0x0000000000000000000000000000000000000000".into(),
        nonce: nonce.into(),
    };
    if matches!(scene, Scene::Delegatecall) {
        t.operation = 1;
        t.to = "0x00000000000000000000000000000000bad1bad1".into();
        t.value = "0x0".into();
    }
    t
}

/// A 2-of-3 of a Vela member here (fixture 0) and two outside owners (A,
/// B); the mailbox holds A's approval of the transaction; the outside wallet
/// holds B. The same world for both machines.
fn world_for(scene: Scene) -> (World, MultisigAccount, SafeTx) {
    let vela = fixture(0);
    let owners = [vela.address.clone(), addr(SEED_A), addr(SEED_B)];
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = world::genesis(&refs, 2, "0x0a");
    let mut w = World {
        locals: vec![vela],
        wallet: Some(WalletSim::holding(&[SEED_B], CHAIN)),
        sim_reply: Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned()),
        ..World::default()
    };
    // The recipient is a contract this device never sent to.
    w.recipients
        .insert(RECIPIENT.to_ascii_lowercase(), (Some(true), Some(true)));
    w.record(&g.address, &g.genesis, false);
    let nonce = if matches!(scene, Scene::RefusedGenesis) {
        w.canonical_missing
            .insert((CHAIN, FACTORY.to_ascii_lowercase()));
        "0x0"
    } else {
        let mut safe = DeployedSafe::clean(&refs, 2);
        safe.nonce = 3;
        if matches!(scene, Scene::Module) {
            safe.modules = vec![MODULE.to_owned()];
        }
        w.deploy(CHAIN, &g.address, safe);
        "0x3"
    };
    let t = tx(scene, nonce);
    let hash = multisig::safe_tx_hash(CHAIN, &g.address, &t).unwrap();
    let a_signs = eoa_sign(
        &eoa(SEED_A).0,
        &vela_core::primitives::from_hex(&hash).unwrap(),
    );
    w.mailbox = vec![json!({
        "chainId": CHAIN, "safe": g.address, "safeTxHash": hash, "safeTx": t,
        "proposer": addr(SEED_A),
        "approvals": [{ "owner": addr(SEED_A), "signature": a_signs, "createdMs": 1 }],
        "createdMs": 1, "updatedMs": 1,
    })];
    if matches!(scene, Scene::Silent) {
        w.rpc_hooks.push(Box::new(|_, m, _| {
            (m == "eth_getStorageAt").then(|| Res::RpcUnanswered {
                detail: "timed out".into(),
            })
        }));
    }
    // Fees, receipts, coin: what this world does not hold itself.
    w.rpc_hooks.push(Box::new(|_, method, _| {
        let ok = |v: Value| Some(Res::RpcResult { result: v });
        match method {
            "eth_gasPrice" => ok(json!("0x3b9aca00")),
            "eth_estimateGas" => ok(json!("0x5208")),
            _ => None,
        }
    }));
    w.risk_signs = false;
    let mut account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[CHAIN], true)).unwrap();
    account.chain_ids = vec![CHAIN];
    (w, account, t)
}

fn run<A>(sut: &mut DomainDriver<A>, w: &mut World, event: A::Event)
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: vela_core::app::SplitEffect<Op = vela_core::app::multisig_shell::MultisigOperation>,
{
    sut.dispatch(event);
    let answered = settle_ready(sut, w, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// What a Vela member's machine comes to: its review, and how its approval
/// ended (the gate's word).
fn as_vela_member(scene: Scene) -> (Value, &'static str, usize) {
    let (mut w, account, t) = world_for(scene);
    let hash = multisig::safe_tx_hash(CHAIN, &account.address, &t).unwrap();
    let mut sut: DomainDriver<MultisigProposals> = DomainDriver::new();
    let start = proposals::Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: w.locals.clone(),
        networks: vec![CHAIN],
        known: Vec::new(),
    };
    run(&mut sut, &mut w, start);
    run(
        &mut sut,
        &mut w,
        proposals::Event::Follow {
            account: Some(account.clone()),
        },
    );
    run(
        &mut sut,
        &mut w,
        proposals::Event::Review {
            chain_id: CHAIN,
            safe_tx_hash: hash.clone(),
            blind: false,
        },
    );
    let review = sut.view().reviews[&format!("{CHAIN}:{hash}")].clone();
    run(
        &mut sut,
        &mut w,
        proposals::Event::Approve {
            chain_id: CHAIN,
            safe_tx_hash: hash.clone(),
            owner: fixture(0).address,
        },
    );
    let gate = match &sut.view().actions[&format!("approve:{hash}")] {
        ActionState::Done { .. } => "open",
        ActionState::RiskDeclined => "risk",
        ActionState::SigningBlocked => "blocked",
        ActionState::SigningUnchecked => "unchecked",
        ActionState::SigningRefused { .. } => "refused",
        other => panic!("{scene:?}: {other:?}"),
    };
    (review, gate, w.risks_asked.len())
}

/// What the connect page comes to for outside owner B: its review, its gate,
/// and whether approving asked the risk (and signed nothing on a no).
fn as_outside_owner(scene: Scene) -> (Value, &'static str, usize) {
    let (mut w, account, t) = world_for(scene);
    let hash = multisig::safe_tx_hash(CHAIN, &account.address, &t).unwrap();
    let mut sut: DomainDriver<OutsidePage> = DomainDriver::new();
    run(
        &mut sut,
        &mut w,
        page::Event::Start {
            service_url: SERVICE.to_owned(),
            wallet_name: "Rabby".to_owned(),
        },
    );
    run(
        &mut sut,
        &mut w,
        page::Event::Link {
            link: TxLink {
                chain_id: CHAIN,
                safe: account.address.clone(),
                safe_tx_hash: hash.clone(),
                for_members: Vec::new(),
            },
        },
    );
    let v = sut
        .view()
        .tx
        .unwrap_or_else(|| panic!("{scene:?}: nothing loaded"));
    let review = v.review.clone().unwrap_or(Value::Null);
    let gate = match &v.gate {
        PageGate::Open => "open",
        PageGate::Risk { .. } => "risk",
        PageGate::Blocked => "blocked",
        PageGate::Unchecked => "unchecked",
        PageGate::Refused { .. } => "refused",
    };
    run(
        &mut sut,
        &mut w,
        page::Event::Approve {
            account: addr(SEED_B),
            form: None,
        },
    );
    let signed = w.wallet.as_ref().unwrap().calls_of("eth_signTypedData_v4");
    assert_eq!(
        signed > 0,
        gate == "open",
        "{scene:?}: the wallet signs only through an open gate"
    );
    (review, gate, w.risks_asked.len())
}

/// The parts of the one review every signer must see alike.
fn shared(review: &Value) -> Value {
    json!({
        "review": review["review"],
        "hashes": review["hashes"],
        "recipients": review["recipients"],
        "state": review["state"],
        "simulation": review["simulation"],
        "simulationSource": review["simulationSource"],
        "concerns": review["concerns"],
        "tone": review["tone"],
        "sign": review["sign"],
    })
}

#[test]
fn a_vela_member_and_an_outside_owner_get_the_same_review_and_the_same_gate() {
    for (scene, expect) in [
        (Scene::Clean, "open"),
        (Scene::Module, "risk"),
        (Scene::Delegatecall, "blocked"),
        (Scene::Silent, "unchecked"),
        (Scene::RefusedGenesis, "refused"),
    ] {
        let (member_review, member_gate, member_asked) = as_vela_member(scene);
        let (page_review, page_gate, page_asked) = as_outside_owner(scene);
        assert_eq!(member_gate, expect, "{scene:?}: the Vela member's gate");
        assert_eq!(page_gate, expect, "{scene:?}: the connect page's gate");
        // A risk is asked once per signature, on both — and a no signs nothing.
        let asked = usize::from(expect == "risk");
        assert_eq!(member_asked, asked, "{scene:?}");
        assert_eq!(page_asked, asked, "{scene:?}");
        if matches!(scene, Scene::Clean | Scene::Module | Scene::Delegatecall) {
            assert!(
                !member_review.is_null() && !page_review.is_null(),
                "{scene:?}: both reviewed"
            );
        }
        if member_review.is_null() || page_review.is_null() {
            assert_eq!(
                member_review.is_null(),
                page_review.is_null(),
                "{scene:?}: a review on one side only"
            );
            continue;
        }
        assert_eq!(
            shared(&member_review),
            shared(&page_review),
            "{scene:?}: the one review"
        );
    }
}

/// The connect page's review now carries what a Vela member's always did:
/// the signing sheet's simulation and the recipient's facts.
#[test]
fn the_connect_pages_review_has_the_simulation_and_the_recipient_checks() {
    let (review, gate, _) = as_outside_owner(Scene::Clean);
    assert_eq!(gate, "open");
    assert_eq!(review["simulationSource"], "simulate", "{review}");
    let recipient = review["recipients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["address"]
                .as_str()
                .is_some_and(|a| a.eq_ignore_ascii_case(RECIPIENT))
        })
        .unwrap_or_else(|| panic!("no recipient: {review}"));
    assert_eq!(recipient["isContract"], true, "{recipient}");
    assert_eq!(recipient["firstTime"], true, "{recipient}");
}
