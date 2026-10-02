//! Spec 097 E (S2) — one rule for a batch's id, through both machines.
//!
//! The desktop pass: PancakeSwap's USDC→BNB batch landed, the desktop had
//! answered `wallet_sendCalls` with the TX hash, and `wallet_getCallsStatus`
//! of that answer read "Unknown bundle id" — the page stayed on "Proceed in
//! your wallet" minutes after the swap. The rule, once, for every shell:
//!
//! - the signing machine answers a batch with its id — its user operation's
//!   hash — the moment the relay accepts it, and with that id however a
//!   later wait ends (never the tx it landed in);
//! - the browser machine knows a batch it answered by the method, and reads
//!   its status on the chain it went to: 200 with the receipt once landed,
//!   100 while pending, 400 when the relay refused it.
//!
//! Driven with the pass's real batch (`req-pcs-usdc-bnb.json`, chain 56) and
//! the real receipt it landed in (`receipt-pcs-usdc-bnb.json`).

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation, DbrShellResult, DbrStoredSite, Event as DbrEvent,
};
use vela_core::app::dapp_permissions::DpermGrant;
use vela_core::app::dapp_rpc;
use vela_core::app::sign_request::{
    ending_of, Event, SignAccountRef, SignApproveOpts, SignEnding, SignOperation as Op,
    SignRecordClose, SignRequest, SignResponsePayload, SignShellResult as Res, SignSubmitOutcome,
};
use vela_core::app::tx_tracker::TrackStatus;

const ORIGIN: &str = "https://pancakeswap.finance";
const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
const TAB: &str = "tab-1";
const NOW: f64 = 1_759_456_751_000.0;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/dapp097/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn batch_params() -> String {
    let request = fixture("req-pcs-usdc-bnb.json");
    assert_eq!(request["method"], "wallet_sendCalls");
    request["params"].to_string()
}

/// The receipt the batch landed in, and the operation's hash in it
/// (`UserOperationEvent`, topic 1).
fn landed() -> (Value, String) {
    let receipt = fixture("receipt-pcs-usdc-bnb.json");
    let op = receipt["logs"]
        .as_array()
        .and_then(|logs| {
            logs.iter().find(|log| {
                log["topics"][0]
                    == "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f"
            })
        })
        .and_then(|log| log["topics"][1].as_str())
        .expect("the operation's event")
        .to_owned();
    (receipt, op)
}

/// `eth_getUserOperationReceipt` as the relay answers it for that op: the
/// operation's verdict, its logs and the bundle transaction's receipt.
fn user_op_receipt(receipt: &Value, op: &str) -> Value {
    json!({
        "userOpHash": op,
        "sender": SAFE,
        "success": true,
        "logs": receipt["logs"],
        "receipt": receipt,
    })
}

type Signer = DomainDriver<SignRequest>;
type Browser = DomainDriver<DappBrowser>;

/// The batch, approved, through the write-ahead: signed as `op`, its record
/// on disk, cleared to POST — the in-app browsers' order (spec 082 RJ1).
fn batch_cleared_to_post(id: &str, op: &str) -> Signer {
    let mut signer = Signer::new();
    signer.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 56, 100],
    });
    signer.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: SAFE.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    signer.dispatch(Event::RequestArrived {
        id: id.to_owned(),
        method: "wallet_sendCalls".to_owned(),
        params_json: batch_params(),
        origin: ORIGIN.to_owned(),
        transport_id: TAB.to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(56),
        dapp: None,
        granted_address: Some(SAFE.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
    });
    signer.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    let ops = signer.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "the batch goes to submit: {ops:?}"
    );
    signer.dispatch(Event::OpSigned {
        id: id.to_owned(),
        user_op_hash: op.to_owned(),
        submit_block: Some(125_336_512),
        now_ms: NOW + 1_000.0,
    });
    let ops = signer.resolve_matching(
        |op| matches!(op, Op::PersistRecord { .. }),
        Res::RecordPersisted,
    );
    assert!(
        matches!(ops.as_slice(), [Op::ClearToPost { .. }]),
        "{ops:?}"
    );
    signer.resolve_matching(|op| matches!(op, Op::ClearToPost { .. }), Res::Responded);
    signer
}

fn accepted(id: &str, op: &str, maybe_sent: bool) -> Event {
    Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: op.to_owned(),
        now_ms: NOW + 2_000.0,
        maybe_sent,
        submit_block: Some(125_336_512),
    }
}

fn answers(ops: &[Op]) -> Vec<SignResponsePayload> {
    ops.iter()
        .filter_map(|op| match op {
            Op::SendResponse { payload, .. } => Some(payload.clone()),
            _ => None,
        })
        .collect()
}

fn ok(result: &str) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(result.to_owned()),
    }
}

/// The page that asked, connected on BNB Chain, with the batch forwarded to
/// signing.
fn browser_with_batch_forwarded() -> Browser {
    let mut browser = Browser::new();
    browser.dispatch(DbrEvent::Start);
    browser.dispatch(DbrEvent::NetworksChanged {
        chain_ids: vec![1, 56, 100],
    });
    browser.dispatch(DbrEvent::AccountsUpdated {
        addresses: Some(vec![SAFE.to_owned()]),
    });
    browser.dispatch(DbrEvent::AccountSwitched {
        address: SAFE.to_owned(),
        now_ms: NOW,
    });
    browser.resolve(DbrShellResult::SitesListed {
        sites: vec![DbrStoredSite {
            origin: ORIGIN.to_owned(),
            grant: Some(DpermGrant {
                origin: ORIGIN.to_owned(),
                address: SAFE.to_owned(),
                chain_id: 56,
                granted_at_ms: NOW,
            }),
            chain_id: None,
        }],
    });
    page(&mut browser, json!({"t":"hello","doc":"d1"}));
    let params: Value = serde_json::from_str(&batch_params()).expect("params");
    let ops = ask(&mut browser, "1", "wallet_sendCalls", params);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DbrOperation::ForwardToSigning { chain_id: 56, .. })),
        "{ops:?}"
    );
    browser
}

fn page(browser: &mut Browser, message: Value) -> Vec<DbrOperation> {
    browser.dispatch(DbrEvent::PageMessage {
        tab: TAB.to_owned(),
        frame_origin: ORIGIN.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
    })
}

fn ask(browser: &mut Browser, id: &str, method: &str, params: Value) -> Vec<DbrOperation> {
    page(
        browser,
        json!({"t":"req","doc":"d1","id":id,"method":method,"params":params}),
    )
}

fn only_answer(ops: &[DbrOperation]) -> Value {
    let answers: Vec<Value> = ops
        .iter()
        .filter_map(|op| match op {
            DbrOperation::Deliver { message_json, .. } => serde_json::from_str(message_json).ok(),
            _ => None,
        })
        .filter(|message: &Value| message["dir"] == "res")
        .collect();
    assert_eq!(answers.len(), 1, "exactly one answer in {ops:?}");
    answers.into_iter().next().expect("one")
}

/// The relay accepted the batch: the page is answered its id — the op hash —
/// at once, the record is patched admitted, and the shell's own receipt wait
/// (which on the desktop ended with the TX hash) is answered by nobody.
#[test]
fn a_batch_is_answered_with_its_id_the_moment_the_relay_takes_it() {
    let (receipt, op) = landed();
    let mut signer = batch_cleared_to_post("rid-1", &op);
    let ops = signer.dispatch(accepted("rid-1", &op, false));
    assert!(
        ops.iter().any(|o| matches!(
            o,
            Op::UpdateRecord {
                close: SignRecordClose::Admitted,
                ..
            }
        )),
        "{ops:?}"
    );
    assert_eq!(answers(&ops), vec![ok(&op)], "the id, once, at acceptance");
    assert!(
        signer.view().tracker_handoff.is_some(),
        "the tracker follows it"
    );
    // The sheet hands over to the ending, which follows the tracker.
    assert_eq!(
        ending_of("wallet_sendCalls", &ok(&op), Some(&op)),
        Some(SignEnding::StillConfirming {
            user_op_hash: op.clone()
        })
    );
    // The shell's own wait lands with the receipt: no second answer.
    let tx = receipt["transactionHash"].as_str().expect("tx").to_owned();
    let late = signer.resolve_matching(
        |o| matches!(o, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::Succeeded { result: tx },
            now_ms: NOW + 12_000.0,
        },
    );
    assert!(answers(&late).is_empty(), "{late:?}");
}

/// A batch the relay may only have been sent (the reply was lost) is
/// answered when its fate is known — and with its id however that is: a
/// receipt in time (the desktop's `Succeeded{tx}`), the tracker's
/// confirmation, or the cap running out. Never the tx hash.
#[test]
fn a_batch_is_never_answered_with_the_tx_it_landed_in() {
    let (receipt, op) = landed();
    let tx = receipt["transactionHash"].as_str().expect("tx").to_owned();
    let outcomes = [
        SignSubmitOutcome::Succeeded { result: tx.clone() },
        SignSubmitOutcome::NotConfirmed {
            user_op_hash: op.clone(),
        },
        SignSubmitOutcome::ReceiptPending {
            user_op_hash: op.clone(),
        },
    ];
    for outcome in outcomes {
        let mut signer = batch_cleared_to_post("rid-2", &op);
        assert!(answers(&signer.dispatch(accepted("rid-2", &op, true))).is_empty());
        let ops = signer.resolve_matching(
            |o| matches!(o, Op::SignAndSubmit { .. }),
            Res::Submit {
                outcome: outcome.clone(),
                now_ms: NOW + 12_000.0,
            },
        );
        assert_eq!(answers(&ops), vec![ok(&op)], "{outcome:?}");
    }
    // The tracker confirms it first (spec 082 RJ4): the id, not its tx.
    let mut signer = batch_cleared_to_post("rid-3", &op);
    signer.dispatch(accepted("rid-3", &op, true));
    let ops = signer.dispatch(Event::OpTracked {
        user_op_hash: op.clone(),
        status: TrackStatus::Confirmed,
        tx_hash: Some(tx),
        now_ms: NOW + 10_000.0,
    });
    assert_eq!(answers(&ops), vec![ok(&op)]);
}

/// A transaction is unchanged: accepted is not landed, and it is answered
/// its tx hash once it lands (083).
#[test]
fn a_transaction_still_waits_for_its_receipt() {
    let (receipt, op) = landed();
    let mut signer = Signer::new();
    signer.dispatch(Event::NetworksChanged {
        chain_ids: vec![56],
    });
    signer.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: SAFE.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    signer.dispatch(Event::RequestArrived {
        id: "rid-4".to_owned(),
        method: "eth_sendTransaction".to_owned(),
        params_json: json!([{"from": SAFE, "to": SAFE, "value": "0x0", "data": "0x"}]).to_string(),
        origin: ORIGIN.to_owned(),
        transport_id: TAB.to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(56),
        dapp: None,
        granted_address: Some(SAFE.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
    });
    signer.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    signer.resolve(Res::PreCheck { funding: None });
    signer.dispatch(Event::OpSigned {
        id: "rid-4".to_owned(),
        user_op_hash: op.clone(),
        submit_block: None,
        now_ms: NOW,
    });
    signer.resolve_matching(
        |o| matches!(o, Op::PersistRecord { .. }),
        Res::RecordPersisted,
    );
    let ops = signer.dispatch(accepted("rid-4", &op, false));
    assert!(answers(&ops).is_empty(), "accepted is not landed: {ops:?}");
    let tx = receipt["transactionHash"].as_str().expect("tx").to_owned();
    let ops = signer.resolve_matching(
        |o| matches!(o, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::Succeeded { result: tx.clone() },
            now_ms: NOW + 12_000.0,
        },
    );
    assert_eq!(answers(&ops), vec![ok(&tx)]);
}

/// End to end: the id the signing machine answered is a batch the browser
/// knows — by the method, with no shell naming the op (the desktop named
/// none) — and its status reads 100 while it is on its way and 200 with the
/// real receipt once it landed, on the chain it went to even after the site
/// moved to another.
#[test]
fn the_answered_id_reads_200_with_its_receipt_after_landing() {
    let (receipt, op) = landed();
    let mut signer = batch_cleared_to_post("1", &op);
    let answer = answers(&signer.dispatch(accepted("1", &op, false)))
        .pop()
        .expect("answered");

    let mut browser = browser_with_batch_forwarded();
    let ops = browser.dispatch(DbrEvent::SigningAnswered {
        tab: TAB.to_owned(),
        id: "1".to_owned(),
        payload: answer,
        user_op_hash: None,
    });
    assert_eq!(
        only_answer(&ops)["result"],
        json!(op),
        "the page has the id"
    );

    // The site moves to Gnosis; the batch is still read where it went.
    browser.dispatch(DbrEvent::SiteChainPicked {
        origin: ORIGIN.to_owned(),
        chain_id: 100,
    });

    // On its way: 100.
    let ops = ask(&mut browser, "2", "wallet_getCallsStatus", json!([op]));
    assert!(
        ops.iter().any(|o| matches!(o, DbrOperation::Read {
            chain_id: 56, method, bundler: true, ..
        } if method == "eth_getUserOperationReceipt")),
        "read on BNB Chain, from the relay: {ops:?}"
    );
    browser.resolve_matching(
        |o| matches!(o, DbrOperation::Read { id, .. } if id == "2"),
        DbrShellResult::ReadAnswered {
            body_json: Some(json!({"result": null}).to_string()),
        },
    );
    let ops = browser.resolve_matching(
        |o| matches!(o, DbrOperation::Read { id, chain_id: 56, .. } if id == "2"),
        DbrShellResult::ReadAnswered {
            body_json: Some(json!({"result": {"status": "submitted"}}).to_string()),
        },
    );
    let pending = only_answer(&ops);
    assert_eq!(pending["result"]["status"], 100);
    assert_eq!(pending["result"]["chainId"], "0x38");

    // Landed: 200, with the receipt the chain gave.
    ask(&mut browser, "3", "wallet_getCallsStatus", json!([op]));
    let ops = browser.resolve_matching(
        |o| matches!(o, DbrOperation::Read { id, chain_id: 56, .. } if id == "3"),
        DbrShellResult::ReadAnswered {
            body_json: Some(json!({"result": user_op_receipt(&receipt, &op)}).to_string()),
        },
    );
    let status = only_answer(&ops)["result"].clone();
    assert_eq!(status["status"], 200, "{status}");
    assert_eq!(status["id"], json!(op));
    assert_eq!(status["atomic"], true);
    assert_eq!(
        status["receipts"][0]["transactionHash"],
        receipt["transactionHash"]
    );
    assert_eq!(status["receipts"][0]["logs"], receipt["logs"]);
    assert_eq!(status["receipts"][0]["status"], "0x1");
}

/// The pass's defect, at the browser: a batch answered with a hash the page
/// can look up — and a TX hash, which is no batch id, stays unknown.
#[test]
fn only_the_batch_id_is_a_known_bundle() {
    let (receipt, op) = landed();
    let tx = receipt["transactionHash"].as_str().expect("tx").to_owned();
    let mut browser = browser_with_batch_forwarded();
    browser.dispatch(DbrEvent::SigningAnswered {
        tab: TAB.to_owned(),
        id: "1".to_owned(),
        payload: ok(&op),
        user_op_hash: None,
    });
    let ops = ask(&mut browser, "2", "wallet_getCallsStatus", json!([tx]));
    assert_eq!(
        only_answer(&ops)["error"]["code"],
        json!(dapp_rpc::UNKNOWN_BUNDLE_ID)
    );
    let ops = ask(&mut browser, "3", "wallet_getCallsStatus", json!([op]));
    assert!(
        ops.iter()
            .any(|o| matches!(o, DbrOperation::Read { id, .. } if id == "3")),
        "{ops:?}"
    );
}
