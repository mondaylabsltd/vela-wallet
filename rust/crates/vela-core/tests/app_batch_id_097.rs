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
//!
//! Spec 097 G (S2) — and in the shape the request declared. Uniswap sent
//! `wallet_sendCalls` with EIP-5792 `version: "2.0.0"`, whose answer is
//! `{ id, capabilities? }`; Vela answered the bare id, Uniswap read `.id`
//! (undefined), polled `wallet_getCallsStatus([null])`, got -32602 four times
//! and said "Failed to swap" over a swap that had landed. Both passes' batches
//! declare 2.0.0, so both are answered `{ "id": <op> }` now; "1.0" and no
//! version keep the bare id. Uniswap's batch: `req-uni-usdt-batch-v2.json`
//! (its three calls decoded from the MultiSend it landed in) and that
//! landing's receipt, `receipt-uni-usdt-batch.json`.

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
    assert_eq!(request["params"][0]["version"], "2.0.0");
    request["params"].to_string()
}

/// The receipt the batch landed in, and the operation's hash in it
/// (`UserOperationEvent`, topic 1).
fn landed() -> (Value, String) {
    landed_in("receipt-pcs-usdc-bnb.json")
}

fn landed_in(name: &str) -> (Value, String) {
    let receipt = fixture(name);
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
    batch_cleared_to_post_with(id, op, &batch_params(), SignApproveOpts::default())
}

fn batch_cleared_to_post_with(
    id: &str,
    op: &str,
    params_json: &str,
    opts: SignApproveOpts,
) -> Signer {
    let mut signer = batch_approved(id, params_json, opts);
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

/// The batch, approved and past the funding check: `SignAndSubmit` is out.
fn batch_approved(id: &str, params_json: &str, opts: SignApproveOpts) -> Signer {
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
        params_json: params_json.to_owned(),
        origin: ORIGIN.to_owned(),
        transport_id: TAB.to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(56),
        dapp: None,
        granted_address: Some(SAFE.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
        first_party: false,
    });
    signer.dispatch(Event::ApproveTapped { opts });
    let ops = signer.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "the batch goes to submit: {ops:?}"
    );
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

/// The bare answer: a tx hash, or a 1.0 batch's id.
fn ok(result: &str) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(json!(result)),
    }
}

/// A 2.0.0 batch's answer (EIP-5792 `SendCallsResult`): `{ id }`, nothing
/// else.
fn ok_id(id: &str) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(json!({ "id": id })),
    }
}

/// The page that asked, connected on BNB Chain, with the batch forwarded to
/// signing.
fn browser_with_batch_forwarded() -> Browser {
    let params: Value = serde_json::from_str(&batch_params()).expect("params");
    browser_with_forwarded(ORIGIN, params)
}

fn browser_with_forwarded(origin: &str, params: Value) -> Browser {
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
            origin: origin.to_owned(),
            grant: Some(DpermGrant {
                origin: origin.to_owned(),
                address: SAFE.to_owned(),
                chain_id: 56,
                granted_at_ms: NOW,
            }),
            chain_id: None,
        }],
    });
    page_from(&mut browser, origin, json!({"t":"hello","doc":"d1"}));
    let ops = ask_from(&mut browser, origin, "1", "wallet_sendCalls", params);
    assert!(
        ops.iter()
            .any(|op| matches!(op, DbrOperation::ForwardToSigning { chain_id: 56, .. })),
        "{ops:?}"
    );
    browser
}

fn page_from(browser: &mut Browser, origin: &str, message: Value) -> Vec<DbrOperation> {
    browser.dispatch(DbrEvent::PageMessage {
        tab: TAB.to_owned(),
        frame_origin: origin.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
        now_ms: 0.0,
    })
}

fn ask(browser: &mut Browser, id: &str, method: &str, params: Value) -> Vec<DbrOperation> {
    ask_from(browser, ORIGIN, id, method, params)
}

fn ask_from(
    browser: &mut Browser,
    origin: &str,
    id: &str,
    method: &str,
    params: Value,
) -> Vec<DbrOperation> {
    page_from(
        browser,
        origin,
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
    // In the shape PancakeSwap's 2.0.0 declared (spec 097 G).
    assert_eq!(
        answers(&ops),
        vec![ok_id(&op)],
        "the id, once, at acceptance"
    );
    assert!(
        signer.view().tracker_handoff.is_some(),
        "the tracker follows it"
    );
    // The sheet hands over to the ending, which follows the tracker.
    assert_eq!(
        ending_of("wallet_sendCalls", &ok_id(&op), Some(&op)),
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
        assert_eq!(answers(&ops), vec![ok_id(&op)], "{outcome:?}");
    }
    // The tracker confirms it first (spec 082 RJ4): the id, not its tx.
    let mut signer = batch_cleared_to_post("rid-3", &op);
    signer.dispatch(accepted("rid-3", &op, true));
    let ops = signer.dispatch(Event::OpTracked {
        user_op_hash: op.clone(),
        status: TrackStatus::Confirmed,
        tx_hash: Some(tx),
        now_ms: NOW + 10_000.0,
        refusal: None,
    });
    assert_eq!(answers(&ops), vec![ok_id(&op)]);
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
        first_party: false,
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
        now_ms: 0.0,
    });
    assert_eq!(
        only_answer(&ops)["result"],
        json!({ "id": op }),
        "the page has the id, in 2.0.0's shape"
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
            failure: None,
            now_ms: 0.0,
        },
    );
    let ops = browser.resolve_matching(
        |o| matches!(o, DbrOperation::Read { id, chain_id: 56, .. } if id == "2"),
        DbrShellResult::ReadAnswered {
            body_json: Some(json!({"result": {"status": "submitted"}}).to_string()),
            failure: None,
            now_ms: 0.0,
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
            failure: None,
            now_ms: 0.0,
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
        now_ms: 0.0,
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

// ---------------------------------------------------------------------------
// Spec 097 G — the answer's shape follows the request's declared version
// ---------------------------------------------------------------------------

const UNISWAP: &str = "https://app.uniswap.org";

/// Uniswap's batch as it asked (`req-uni-usdt-batch-v2.json`): USDT approve
/// → Permit2, Permit2 approve, the swap — an unbounded USDT approval the
/// person kept as asked (the sheet's consent, `unlimited_approved`).
fn uniswap_params() -> Value {
    let request = fixture("req-uni-usdt-batch-v2.json");
    assert_eq!(request["method"], "wallet_sendCalls");
    assert_eq!(request["params"][0]["version"], "2.0.0");
    assert_eq!(
        request["params"][0]["calls"].as_array().map(Vec::len),
        Some(3)
    );
    request["params"].clone()
}

fn kept_as_asked() -> SignApproveOpts {
    SignApproveOpts {
        unlimited_approved: true,
        ..SignApproveOpts::default()
    }
}

/// `params` with its `version` set to `version` (`None`: removed).
fn declaring(mut params: Value, version: Option<&str>) -> Value {
    let first = params[0].as_object_mut().expect("an object");
    match version {
        Some(version) => first.insert("version".to_owned(), json!(version)),
        None => first.remove("version"),
    };
    params
}

/// The page's answer to a batch declaring `version`, at acceptance.
fn answer_at_acceptance(params: &Value, op: &str) -> Vec<SignResponsePayload> {
    let mut signer = batch_cleared_to_post_with("rid-g", op, &params.to_string(), kept_as_asked());
    answers(&signer.dispatch(accepted("rid-g", op, false)))
}

/// The pass's defect: Uniswap declared 2.0.0 and was answered the bare
/// string. Now `{ "id": <op> }` — EIP-5792's `SendCallsResult`, with no
/// `capabilities` (Vela attaches none) — and the sheet reads the id out of it.
#[test]
fn a_2_0_0_batch_is_answered_an_object_carrying_its_id() {
    let (_, op) = landed_in("receipt-uni-usdt-batch.json");
    let request = fixture("req-uni-usdt-batch-v2.json");
    assert_eq!(
        request["user_op_hash"],
        json!(op),
        "the receipt is the batch's"
    );
    let answer = answer_at_acceptance(&uniswap_params(), &op);
    assert_eq!(answer, vec![ok_id(&op)]);
    let SignResponsePayload::Ok {
        result: Some(result),
    } = &answer[0]
    else {
        unreachable!("an answer");
    };
    assert_eq!(
        result
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>()),
        Some(vec!["id".to_owned()]),
        "{result}"
    );
    assert_eq!(answer[0].answered(), Some(op.as_str()));
    assert_eq!(
        ending_of("wallet_sendCalls", &answer[0], Some(&op)),
        Some(SignEnding::StillConfirming {
            user_op_hash: op.clone()
        })
    );
}

/// The other half: "1.0", no version, or one that is not a number keeps the
/// bare id — dApps built on EIP-5792 1.0 read a string.
#[test]
fn a_1_0_batch_or_one_with_no_version_keeps_the_bare_id() {
    let (_, op) = landed_in("receipt-uni-usdt-batch.json");
    for version in [Some("1.0"), Some("1.0.0"), None, Some("v2")] {
        let params = declaring(uniswap_params(), version);
        assert_eq!(
            answer_at_acceptance(&params, &op),
            vec![ok(&op)],
            "{version:?}"
        );
    }
    for version in ["2.0", "3.0.0"] {
        let params = declaring(uniswap_params(), Some(version));
        assert_eq!(
            answer_at_acceptance(&params, &op),
            vec![ok_id(&op)],
            "{version}"
        );
    }
}

/// The shape is the request's, as the page SENT it: a sheet that rewrote the
/// params (the approval guard's cap) and lost the version on the way does
/// not change what the page parses.
#[test]
fn the_shape_follows_what_the_page_sent_not_the_rewrite() {
    let (_, op) = landed_in("receipt-uni-usdt-batch.json");
    let rewrite = declaring(uniswap_params(), None).to_string();
    let opts = SignApproveOpts {
        params_override_json: Some(rewrite),
        ..kept_as_asked()
    };
    let mut signer = batch_cleared_to_post_with("rid-r", &op, &uniswap_params().to_string(), opts);
    assert_eq!(
        answers(&signer.dispatch(accepted("rid-r", &op, false))),
        vec![ok_id(&op)]
    );
}

/// A shell that reports the batch with no write-ahead — the id as its
/// result — is answered after the record lands (§4), in the same shape; the
/// record names the bare id, which Activity and the tracker follow.
#[test]
fn a_batch_answered_after_its_record_takes_the_same_shape() {
    let (_, op) = landed_in("receipt-uni-usdt-batch.json");
    let mut signer = batch_approved("rid-l", &uniswap_params().to_string(), kept_as_asked());
    let ops = signer.resolve_matching(
        |o| matches!(o, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: op.clone(),
            },
            now_ms: NOW + 3_000.0,
        },
    );
    let record = ops
        .iter()
        .find_map(|o| match o {
            Op::PersistRecord { record } => Some(record.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the record first: {ops:?}"));
    assert_eq!(record.user_op_hash, op, "the record names the bare id");
    assert!(!record.result.contains('{'), "{}", record.result);
    assert!(answers(&ops).is_empty(), "not before the record: {ops:?}");
    let ops = signer.resolve_matching(
        |o| matches!(o, Op::PersistRecord { .. }),
        Res::RecordPersisted,
    );
    assert_eq!(answers(&ops), vec![ok_id(&op)]);
}

/// Uniswap, end to end through both machines: the page reads `.id` off the
/// answer — as viem and Uniswap do — and `wallet_getCallsStatus(id)` reads
/// 200 with the receipt the swap landed in. `[null]` (what Uniswap asked
/// with) stays -32602; the whole answer handed back, `[{ id }]`, reads the
/// same batch.
#[test]
fn uniswap_reads_the_id_off_the_answer_and_its_status_is_200() {
    let (receipt, op) = landed_in("receipt-uni-usdt-batch.json");
    let mut signer =
        batch_cleared_to_post_with("1", &op, &uniswap_params().to_string(), kept_as_asked());
    let answer = answers(&signer.dispatch(accepted("1", &op, false)))
        .pop()
        .expect("answered");

    let mut browser = browser_with_forwarded(UNISWAP, uniswap_params());
    let ops = browser.dispatch(DbrEvent::SigningAnswered {
        tab: TAB.to_owned(),
        id: "1".to_owned(),
        payload: answer,
        user_op_hash: None,
        now_ms: 0.0,
    });
    let id = only_answer(&ops)["result"]["id"].clone();
    assert_eq!(id, json!(op), "the page reads `.id`");

    // What Uniswap asked with when `.id` was undefined: refused, as before.
    let ops = ask_from(
        &mut browser,
        UNISWAP,
        "2",
        "wallet_getCallsStatus",
        json!([null]),
    );
    let refused = only_answer(&ops);
    assert_eq!(refused["error"]["code"], -32602, "{refused}");

    // `wallet_getCallsStatus([id])`: 200 with the swap's receipt.
    ask_from(
        &mut browser,
        UNISWAP,
        "3",
        "wallet_getCallsStatus",
        json!([id]),
    );
    let ops = browser.resolve_matching(
        |o| {
            matches!(o, DbrOperation::Read { id, chain_id: 56, method, .. }
            if id == "3" && method == "eth_getUserOperationReceipt")
        },
        DbrShellResult::ReadAnswered {
            body_json: Some(json!({"result": user_op_receipt(&receipt, &op)}).to_string()),
            failure: None,
            now_ms: 0.0,
        },
    );
    let status = only_answer(&ops)["result"].clone();
    assert_eq!(status["status"], 200, "{status}");
    assert_eq!(status["id"], json!(op));
    assert_eq!(status["version"], "2.0.0");
    assert_eq!(
        status["receipts"][0]["transactionHash"],
        json!("0xc4566172c559d3db846d2dcf222f36f0f6d469026b68832301635fc756821482")
    );

    // The answer handed back whole is the same batch.
    let ops = ask_from(
        &mut browser,
        UNISWAP,
        "4",
        "wallet_getCallsStatus",
        json!([{ "id": id }]),
    );
    assert!(
        ops.iter()
            .any(|o| matches!(o, DbrOperation::Read { id, chain_id: 56, .. } if id == "4")),
        "{ops:?}"
    );
}
