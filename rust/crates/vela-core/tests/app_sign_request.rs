//! Rules of the sign_request machine, one test per rule.
//!
//! Inventory invariants ①–⑩ (specs/016-crux-wallet-state/inventory.md,
//! "### sign_request") each have at least one test named after the rule; the
//! driver exercises the machine exactly as the wasm shell will — dispatch an
//! event, answer the operations one at a time.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::fee_policy::{tempo_reimbursement, FeeTier, TEMPO_FEE_TOKEN_DECIMALS};
use vela_core::app::sign_request::{
    dapp_receipt_wait_ms, ending_of, ending_state, extract_request_chain_id, is_signing_method,
    method_kind, required_capabilities, sign_account_index, Event, SignAccountRef, SignApproveOpts,
    SignDappIdentity, SignEnding, SignEndingState, SignErrorKind, SignFundingNeeded,
    SignFundingPresentation, SignMethodKind, SignNotice, SignOperation as Op, SignPhase,
    SignQuotedFee, SignRecord, SignRecordClose, SignRecordKind, SignRecordStatus, SignRequest,
    SignResponsePayload, SignSettledOutcome, SignShellResult as Res, SignSponsorship,
    SignSubmitOutcome, SignSurface, SignSwipeAction, SignTrackerHandoff, SignTrackerWithdraw,
    CODE_INTERNAL, CODE_INVALID_PARAMS, CODE_UNAUTHORIZED, CODE_UNSUPPORTED_CAPABILITY,
    CODE_UNSUPPORTED_CHAIN, CODE_USER_REJECTED, DAPP_TX_ANSWER_WINDOW_MS, EXTENSION_REQUEST_TTL_MS,
};
use vela_core::app::tx_tracker::{TrackEntryView, TrackOutcome, TrackStatus};

type Sut = DomainDriver<SignRequest>;

const WP: &str = "wp-1"; // the durable WalletPair transport
const EXT: &str = "ext-9"; // a one-shot extension/popup transport
const ORIGIN: &str = "https://dapp.example";
const TOKEN: &str = "0x2222222222222222222222222222222222222222";
const SPENDER: &str = "0x3333333333333333333333333333333333333333";
const ACCT0: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ACCT1: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NOW: f64 = 1_700_000_000_000.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn accounts() -> Vec<SignAccountRef> {
    vec![
        SignAccountRef {
            address: ACCT0.to_owned(),
            credential_id: "cred-0".to_owned(),
        },
        SignAccountRef {
            address: ACCT1.to_owned(),
            credential_id: "cred-1".to_owned(),
        },
    ]
}

/// A machine with networks + accounts loaded — the shell's boot sequence.
fn boot() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 137, 4_217],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: accounts(),
        active_index: 0,
    });
    sut
}

struct Arrive {
    id: String,
    method: String,
    params_json: String,
    transport_id: String,
    dedicated: bool,
    per_request_chain: Option<u32>,
    dapp: Option<SignDappIdentity>,
    granted: Option<String>,
    requested: Option<String>,
    ts: Option<f64>,
}

impl Arrive {
    fn global(id: &str, method: &str, params_json: &str) -> Self {
        Self {
            id: id.to_owned(),
            method: method.to_owned(),
            params_json: params_json.to_owned(),
            transport_id: WP.to_owned(),
            dedicated: false,
            per_request_chain: None,
            dapp: None,
            granted: None,
            requested: None,
            ts: None,
        }
    }

    fn extension(id: &str, method: &str, params_json: &str, chain: u32) -> Self {
        Self {
            id: id.to_owned(),
            method: method.to_owned(),
            params_json: params_json.to_owned(),
            transport_id: EXT.to_owned(),
            dedicated: true,
            per_request_chain: Some(chain),
            dapp: Some(SignDappIdentity {
                name: "app.example".to_owned(),
                url: Some("https://app.example".to_owned()),
            }),
            granted: None,
            requested: None,
            ts: None,
        }
    }

    fn event(self) -> Event {
        Event::RequestArrived {
            id: self.id,
            method: self.method,
            params_json: self.params_json,
            origin: ORIGIN.to_owned(),
            transport_id: self.transport_id,
            dedicated_transport: self.dedicated,
            per_request_chain: self.per_request_chain,
            dapp: self.dapp,
            granted_address: self.granted,
            requested_address: self.requested,
            request_ts_ms: self.ts,
            now_ms: NOW,
        }
    }
}

fn tx_params(data: &str) -> String {
    format!(r#"[{{"to":"{TOKEN}","data":"{data}","value":"0x0"}}]"#)
}

fn plain_send_params() -> String {
    format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0xde0b6b3a7640000"}}]"#)
}

/// `approve(address,uint256)` calldata; amount as a 64-hex-digit word.
fn approve_calldata(amount_word: &str) -> String {
    format!(
        "0x095ea7b3{:0>64}{amount_word:0>64}",
        SPENDER.trim_start_matches("0x")
    )
}

fn unlimited_approve_params() -> String {
    tx_params(&approve_calldata(&"f".repeat(64)))
}

fn capped_approve_params() -> String {
    tx_params(&approve_calldata("de0b6b3a7640000"))
}

fn approve(opts: SignApproveOpts) -> Event {
    Event::ApproveTapped { opts }
}

fn funding_fixture() -> SignFundingNeeded {
    SignFundingNeeded {
        deposit_address: "0x4444444444444444444444444444444444444444".to_owned(),
        safe_address: ACCT0.to_owned(),
        chain_id: 1,
        native_symbol: "ETH".to_owned(),
        threshold_wei: "200000000000000".to_owned(),
        recommended_wei: "500000000000000".to_owned(),
        current_balance_wei: "100000000000000".to_owned(),
    }
}

fn submit_ok(result: &str) -> Res {
    Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: result.to_owned(),
        },
        now_ms: NOW + 1_000.0,
    }
}

/// Extract the (code, kind) of a SendResponse error op.
fn response_error(op: &Op) -> Option<(i32, SignErrorKind, String)> {
    match op {
        Op::SendResponse {
            transport_id,
            payload: SignResponsePayload::Err { code, kind, .. },
            ..
        } => Some((*code, *kind, transport_id.clone())),
        _ => None,
    }
}

fn response_ok(op: &Op) -> Option<(String, Option<String>)> {
    match op {
        Op::SendResponse {
            transport_id,
            payload: SignResponsePayload::Ok { result },
            ..
        } => Some((transport_id.clone(), result.clone())),
        _ => None,
    }
}

// ===========================================================================
// Pure helper ports
// ===========================================================================

#[test]
fn signing_method_classification_matches_ts() {
    assert!(is_signing_method("eth_sendTransaction"));
    assert!(is_signing_method("wallet_sendCalls"));
    assert!(is_signing_method("personal_sign"));
    assert!(is_signing_method("eth_sign"));
    assert!(is_signing_method("eth_signTypedData_v4"));
    assert!(!is_signing_method("eth_accounts"));
    assert!(!is_signing_method("eth_getBalance"));
    assert_eq!(method_kind("wallet_sendCalls"), SignMethodKind::Batch);
    assert_eq!(method_kind("eth_signTypedData"), SignMethodKind::TypedData);
    assert_eq!(method_kind("eth_sign"), SignMethodKind::EthSign);
}

#[test]
fn extract_chain_reads_typed_data_tx_and_batch_shapes() {
    // _v4 order: [address, typedData]; hex string chain.
    let typed = serde_json::json!(["0x0", r#"{"domain":{"chainId":"0x89"}}"#]);
    assert_eq!(
        extract_request_chain_id("eth_signTypedData_v4", &typed),
        Some(137)
    );
    // unsuffixed order: [typedData, address]; numeric chain.
    let typed_v1 = serde_json::json!([{ "domain": { "chainId": 137 } }, "0x0"]);
    assert_eq!(
        extract_request_chain_id("eth_signTypedData", &typed_v1),
        Some(137)
    );
    let tx = serde_json::json!([{ "chainId": "0x1", "to": "0x0" }]);
    assert_eq!(
        extract_request_chain_id("eth_sendTransaction", &tx),
        Some(1)
    );
    let batch = serde_json::json!([{ "chainId": "137", "calls": [] }]);
    assert_eq!(
        extract_request_chain_id("wallet_sendCalls", &batch),
        Some(137)
    );
    // No hint / malformed → None.
    let none = serde_json::json!([{ "to": "0x0" }]);
    assert_eq!(extract_request_chain_id("eth_sendTransaction", &none), None);
    assert_eq!(
        extract_request_chain_id("personal_sign", &serde_json::json!(["0xdead", "0x0"])),
        None
    );
}

#[test]
fn required_capabilities_default_required_unless_optional_true() {
    let payload = serde_json::json!({
        "capabilities": { "paymasterService": {}, "extra": { "optional": true } },
        "calls": [ { "capabilities": { "auxiliaryFunds": { "optional": false } } } ],
    });
    assert_eq!(
        required_capabilities(&payload),
        vec!["auxiliaryFunds".to_owned(), "paymasterService".to_owned()]
    );
    let clean = serde_json::json!({ "calls": [ { "to": "0x0" } ] });
    assert!(required_capabilities(&clean).is_empty());
}

#[test]
fn sign_account_index_prefers_granted_falls_back_visible() {
    let accts = accounts();
    assert_eq!(sign_account_index(&accts, 0, Some(ACCT1)), 1);
    // Case-insensitive.
    assert_eq!(
        sign_account_index(&accts, 0, Some(&ACCT1.to_uppercase().replace("0X", "0x"))),
        1
    );
    // Unknown grant → keep the active signer VISIBLE, never silent.
    assert_eq!(
        sign_account_index(
            &accts,
            1,
            Some("0x9999999999999999999999999999999999999999")
        ),
        1
    );
    assert_eq!(sign_account_index(&accts, 1, None), 1);
}

// ===========================================================================
// ① BUG-2 — a rejected request never submits / never double-responds
// ===========================================================================

#[test]
fn bug2_reject_during_gas_precheck_aborts_before_submit() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-1", "eth_sendTransaction", &plain_send_params()).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::CheckBundlerFunding {
                chain_id: 1,
                bust_cache: false,
                ..
            }]
        ),
        "approve starts at the funding pre-check: {ops:?}"
    );

    // Reject while the ≤15s pre-check is in flight — 4001 goes out now.
    let ops = sut.dispatch(Event::RejectTapped);
    assert_eq!(ops.len(), 1);
    let (code, kind, tid) = response_error(&ops[0]).expect("a 4001 response");
    assert_eq!(
        (code, kind, tid),
        (
            CODE_USER_REJECTED,
            SignErrorKind::UserRejected,
            WP.to_owned()
        )
    );

    // The late pre-check answer must never reach submission (BUG-2).
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(ops.is_empty(), "a rejected pipeline never submits: {ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn bug2_swipe_after_commit_dismisses_and_late_result_still_delivers() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-2", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]));
    assert_eq!(
        sut.view().swipe_action,
        SignSwipeAction::Dismiss,
        "committed → dismiss"
    );

    // Swipe now: NO 4001, the op proceeds.
    let ops = sut.dispatch(Event::SwipeDismissed);
    assert!(ops.is_empty(), "dismiss sends nothing: {ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);

    // The real result is still recorded and delivered to the dApp (§4 order:
    // record first, then the pollable result).
    let ops = sut.resolve(submit_ok("0xtxhash"));
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.status == SignRecordStatus::Confirmed && record.result == "0xtxhash"),
        "record precedes the result: {ops:?}"
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xtxhash".to_owned())))
    );
}

#[test]
fn bug2_reject_is_ignored_once_committed() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-3", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]));
    // A stray reject event past the commitment point must not 4001.
    let ops = sut.dispatch(Event::RejectTapped);
    assert!(ops.is_empty(), "no 4001 after commit: {ops:?}");
}

// ===========================================================================
// ② BUG-3 — a same-tick double tap never runs two pipelines
// ===========================================================================

#[test]
fn bug3_same_tick_double_approve_is_single_flight() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-4", "eth_sendTransaction", &plain_send_params()).event());
    let first = sut.dispatch(approve(SignApproveOpts::default()));
    assert_eq!(first.len(), 1, "one pre-check");
    let second = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        second.is_empty(),
        "the second tap finds the pipeline occupied: {second:?}"
    );
    assert_eq!(
        sut.outstanding().len(),
        1,
        "exactly one operation in flight"
    );
}

// ===========================================================================
// ③ funding — same rid, original capped opts; no hijack of newer requests
// ===========================================================================

fn opts_with_override(override_json: &str) -> SignApproveOpts {
    SignApproveOpts {
        params_override_json: Some(override_json.to_owned()),
        ..SignApproveOpts::default()
    }
}

#[test]
fn funding_retry_replays_same_rid_with_original_capped_opts() {
    let capped = capped_approve_params();
    let mut sut = boot();
    sut.dispatch(
        Arrive::global("req-5", "eth_sendTransaction", &unlimited_approve_params()).event(),
    );
    sut.dispatch(approve(opts_with_override(&capped)));

    let ops = sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    assert!(matches!(
        ops.as_slice(),
        [Op::AttemptSponsorship { force: false, .. }]
    ));
    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Denied {
            reason: Some("budget".to_owned()),
        },
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Funding);
    let funding = view.funding.expect("funding view");
    assert_eq!(funding.presentation, SignFundingPresentation::Topup);
    assert_eq!(funding.denial_reason.as_deref(), Some("budget"));

    // Continue after top-up: fresh cache, SAME capped params all the way in.
    let ops = sut.dispatch(Event::FundingCompleteTapped);
    assert!(
        matches!(
            ops.as_slice(),
            [Op::CheckBundlerFunding {
                bust_cache: true,
                ..
            }]
        ),
        "retry busts the bundler cache: {ops:?}"
    );
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { id, params_json, .. }]
            if id == "req-5" && params_json == &capped),
        "the retry submits the SAME rid with the capped params: {ops:?}"
    );
}

#[test]
fn late_funding_outcome_never_hijacks_a_newer_request() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-A", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });

    // A fresh request takes the sheet while sponsorship runs.
    sut.dispatch(Arrive::global("req-B", "personal_sign", r#"["0xdead","0x0"]"#).event());

    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Denied { reason: None },
    });
    // The superseded pipeline is answered (fail-closed), the funding view
    // does NOT cover the new request.
    let (code, kind, _) = response_error(&ops[0]).expect("superseded pipeline answered");
    assert_eq!((code, kind), (CODE_INTERNAL, SignErrorKind::SubmitFailed));
    let view = sut.view();
    assert!(view.funding.is_none(), "no hijack of the new request");
    assert_eq!(view.request.expect("new request").id, "req-B");
    assert!(view.confirm_gate_open);
}

#[test]
fn fresh_request_clears_leftover_funding_and_pin() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-C", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Denied { reason: None },
    });
    assert_eq!(sut.view().surface, SignSurface::Funding);

    // A fresh signing request supersedes the leftover funding prompt.
    sut.dispatch(Arrive::global("req-D", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    // The stale pin cannot replay old opts under the new request.
    let ops = sut.dispatch(Event::FundingCompleteTapped);
    assert!(
        ops.is_empty(),
        "no funding view → nothing to complete: {ops:?}"
    );
}

// ===========================================================================
// ④ §4 — the durable record precedes the pollable result
// ===========================================================================

#[test]
fn signature_record_lands_before_the_response() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-6", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "no gas pre-check for signatures"
    );

    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: "0xsig".to_owned(),
        },
        now_ms: 1_000.0,
    });
    // Record FIRST — no response yet.
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.record_id == "dapp-1000-msg"
                && record.kind == SignRecordKind::SignMessage
                && record.status == SignRecordStatus::Confirmed
                && record.result == "0xsig"),
        "durable record precedes the result: {ops:?}"
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xsig".to_owned())))
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

/// The pending record lands at submission; the final answer goes to the page
/// and the record is left to the tracker (spec 082 T023, RA8 3 — before 082
/// this machine flipped it confirmed itself, racing the tracker's verdict).
#[test]
fn tx_pending_record_persists_at_submission_and_only_the_tracker_closes_it() {
    let capped = capped_approve_params();
    let mut sut = boot();
    sut.dispatch(
        Arrive::global("req-7", "eth_sendTransaction", &unlimited_approve_params()).event(),
    );
    sut.dispatch(approve(opts_with_override(&capped)));
    sut.resolve(Res::PreCheck { funding: None });

    // Bundler accepted — pending record BEFORE the receipt wait.
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "req-7".to_owned(),
        user_op_hash: "0xophash".to_owned(),
        now_ms: 5_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.record_id == "dapp-5000-tx"
                && record.status == SignRecordStatus::Pending
                && record.user_op_hash == "0xophash"
                && record.result.is_empty()
                && record.params_json == capped),
        "pending record with the CAPPED params: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.pending_op_hash.as_deref(), Some("0xophash"));
    let handoff = view.tracker_handoff.expect("tx_tracker handoff");
    assert_eq!(handoff.user_op_hash, "0xophash");
    assert_eq!(handoff.record_ids, vec!["dapp-5000-tx".to_owned()]);
    assert_eq!(handoff.chain_id, 1);

    // Final result: the page is answered; the record is the tracker's to close.
    let ops = sut.resolve(Res::RecordPersisted);
    assert!(ops.is_empty(), "the pending record's ack moves nothing");
    let ops = sut.resolve(submit_ok("0xtxhash"));
    assert_eq!(ops.len(), 1, "the answer only: {ops:?}");
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xtxhash".to_owned())))
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "no Confirmed patch from sign_request: {ops:?}"
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

/// Issue 262: the receipt wait timed out. The page still gets the op hash,
/// but the record stays PENDING — no confirming patch — and the tracker (which
/// already holds the op) is the only one who may close it.
#[test]
fn receipt_timeout_answers_the_op_hash_and_leaves_the_record_pending() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-7t", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.dispatch(Event::OpSubmitted {
        id: "req-7t".to_owned(),
        user_op_hash: "0xophash".to_owned(),
        now_ms: 5_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    sut.resolve(Res::RecordPersisted);

    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: "0xophash".to_owned(),
        },
        now_ms: 125_000.0,
    });
    assert_eq!(ops.len(), 1, "only the answer — no record patch: {ops:?}");
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xophash".to_owned())))
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "a late receipt never confirms the record: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden);
    let handoff = view
        .tracker_handoff
        .expect("the tracker still holds the op");
    assert_eq!(handoff.user_op_hash, "0xophash");
    assert_eq!(handoff.record_ids, vec!["dapp-5000-tx".to_owned()]);
    assert!(
        sut.resolve(Res::Responded).is_empty(),
        "the answer's ack closes nothing"
    );
    assert!(sut.outstanding().is_empty(), "nothing left to close");
}

/// Without an `OpSubmitted` the late-receipt answer still lands a durable
/// record first — PENDING under the op hash, handed to the tracker.
#[test]
fn receipt_timeout_without_op_submitted_persists_pending_then_answers() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-7u", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: "0xophash".to_owned(),
        },
        now_ms: 9_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.record_id == "dapp-9000-tx"
                && record.status == SignRecordStatus::Pending
                && record.user_op_hash == "0xophash"
                && record.result.is_empty()),
        "pending record before the answer: {ops:?}"
    );
    let handoff = sut.view().tracker_handoff.expect("handoff");
    assert_eq!(handoff.record_ids, vec!["dapp-9000-tx".to_owned()]);
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xophash".to_owned())))
    );
    assert!(!ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })));
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

/// End to end across the two machines: the record the sign machine left
/// pending is closed by the tracker when the receipt finally lands — with
/// the REAL tx hash, on the same record id.
#[test]
fn a_record_left_pending_by_a_late_receipt_is_closed_by_the_tracker() {
    use vela_core::app::tx_tracker::{
        Event as TrackEvent, TrackOperation as TOp, TrackRecordPatch, TrackRecordStatus,
        TrackShellResult as TRes, TxTracker, WAIT_WINDOW_MS,
    };
    let op_hash = "0xabababababababababababababababababababababababababababababababab";
    let tx_hash = "0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

    let mut sign = boot();
    sign.dispatch(Arrive::global("req-7v", "eth_sendTransaction", &plain_send_params()).event());
    sign.dispatch(approve(SignApproveOpts::default()));
    sign.resolve(Res::PreCheck { funding: None });
    sign.dispatch(Event::OpSubmitted {
        id: "req-7v".to_owned(),
        user_op_hash: op_hash.to_owned(),
        now_ms: NOW,
        maybe_sent: false,
        submit_block: None,
    });
    sign.resolve(Res::RecordPersisted);
    let handoff = sign.view().tracker_handoff.expect("handoff");

    // The shell feeds the handoff to the tracker.
    let mut tracker = support::DomainDriver::<TxTracker>::new();
    tracker.dispatch(TrackEvent::Submitted {
        user_op_hash: handoff.user_op_hash.clone(),
        record_ids: handoff.record_ids.clone(),
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    });
    tracker.resolve(TRes::Clock { now_ms: NOW });
    tracker.resolve(TRes::ReceiptPending {
        user_op_hash: op_hash.to_owned(),
        now_ms: NOW + 300.0,
    });

    // The sign executor's own wait runs out: the page gets the op hash, and
    // the record is left alone.
    let ops = sign.resolve(Res::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: op_hash.to_owned(),
        },
        now_ms: NOW + WAIT_WINDOW_MS,
    });
    assert!(!ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })));

    // Minutes later the tracker's reconcile-paced poll finds the receipt.
    let ops = tracker.dispatch(TrackEvent::Tick);
    assert_eq!(ops, vec![TOp::Now]);
    let ops = tracker.resolve(TRes::Clock {
        now_ms: NOW + WAIT_WINDOW_MS + 60_000.0,
    });
    assert!(
        ops.iter().any(|op| matches!(op, TOp::PollReceipt { .. })),
        "{ops:?}"
    );
    let ops = tracker.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::Receipt {
            user_op_hash: op_hash.to_owned(),
            tx_hash: tx_hash.to_owned(),
            now_ms: NOW + WAIT_WINDOW_MS + 61_000.0,
        },
    );
    assert!(
        ops.contains(&TOp::UpdateTxRecords {
            ids: handoff.record_ids.clone(),
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Confirmed,
                tx_hash: Some(tx_hash.to_owned()),
            },
        }),
        "the tracker closes the SAME record with the real tx hash: {ops:?}"
    );
}

/// Spec 082 RA8 (was: sign_request patched the record failed and answered
/// -32603 — "no eternal pending"). Past `OpSubmitted` the tracker holds the
/// record and alone closes it: a dropped op's failed receipt patches it
/// failed there, so nothing is left pending forever. The page gets the op
/// hash, never a failure it would retry over an op the relay accepted.
#[test]
fn a_failure_after_the_hand_off_leaves_the_record_to_the_tracker() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-8", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.dispatch(Event::OpSubmitted {
        id: "req-8".to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 6_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    let handoff = sut.view().tracker_handoff.expect("the tracker holds it");
    assert_eq!(handoff.record_ids, vec!["dapp-6000-tx".to_owned()]);
    assert!(sut.resolve(Res::RecordPersisted).is_empty());
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "dropped from the network".to_owned(),
            refused: false,
        },
        now_ms: 7_000.0,
    });
    assert_eq!(ops.len(), 1, "no record patch from here: {ops:?}");
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xop".to_owned())))
    );
    let view = sut.view();
    assert!(view.error.is_none());
    assert_eq!(view.surface, SignSurface::Hidden);
}

// ===========================================================================
// ⑤ F2/F3/F4 — per-request transport, chain and identity
// ===========================================================================

#[test]
fn f2_response_routes_to_the_owning_transport() {
    let mut sut = boot();
    sut.dispatch(Arrive::extension("rid-1", "personal_sign", r#"["0xdead","0x0"]"#, 137).event());
    let ops = sut.dispatch(Event::RejectTapped);
    let (code, _, tid) = response_error(&ops[0]).expect("response");
    assert_eq!(
        (code, tid),
        (CODE_USER_REJECTED, EXT.to_owned()),
        "owner transport, never a shared ref"
    );
}

#[test]
fn f3_f4_request_uses_its_own_chain_and_dapp_identity() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-2", "eth_sendTransaction", &plain_send_params(), 137).event(),
    );
    let view = sut.view();
    let request = view.request.expect("request");
    assert_eq!(request.chain_id, 137, "the origin's granted chain (F4)");
    assert_eq!(view.global_chain_id, 1, "the global chain is untouched");

    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(matches!(
        ops.as_slice(),
        [Op::CheckBundlerFunding { chain_id: 137, .. }]
    ));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(matches!(
        ops.as_slice(),
        [Op::SignAndSubmit { chain_id: 137, .. }]
    ));
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "rid-2".to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 8_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.chain_id == 137 && record.dapp_origin == "app.example"),
        "history carries the request's own chain + identity (F3): {ops:?}"
    );
}

// ===========================================================================
// ⑥ chain switching
// ===========================================================================

#[test]
fn chain_switch_cancels_global_chain_pending_with_4001() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-9", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let ops = sut.dispatch(Event::ChainSwitchRequested {
        id: Some("sw-1".to_owned()),
        transport_id: Some(WP.to_owned()),
        chain_id_param: Some("0x89".to_owned()),
    });
    assert_eq!(ops.len(), 2);
    let (code, kind, _) = response_error(&ops[0]).expect("cancellation");
    assert_eq!(
        (code, kind),
        (CODE_USER_REJECTED, SignErrorKind::WalletSwitchedChains)
    );
    assert_eq!(
        response_ok(&ops[1]),
        Some((WP.to_owned(), None)),
        "switch answered null"
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden);
    assert_eq!(view.global_chain_id, 137);
}

#[test]
fn chain_switch_leaves_per_request_extension_sign_intact() {
    let mut sut = boot();
    sut.dispatch(Arrive::extension("rid-3", "personal_sign", r#"["0xdead","0x0"]"#, 137).event());
    let ops = sut.dispatch(Event::ChainSwitchRequested {
        id: Some("sw-2".to_owned()),
        transport_id: Some(WP.to_owned()),
        chain_id_param: Some("1".to_owned()),
    });
    assert_eq!(ops.len(), 1, "only the switch ack: {ops:?}");
    assert_eq!(sut.view().request.expect("kept").id, "rid-3");
}

#[test]
fn chain_switch_missing_or_malformed_param_is_32602_never_phantom_success() {
    let mut sut = boot();
    for bad in [None, Some("nonsense".to_owned())] {
        let ops = sut.dispatch(Event::ChainSwitchRequested {
            id: Some("sw-3".to_owned()),
            transport_id: Some(WP.to_owned()),
            chain_id_param: bad,
        });
        let (code, kind, _) = response_error(&ops[0]).expect("error");
        assert_eq!(
            (code, kind),
            (CODE_INVALID_PARAMS, SignErrorKind::InvalidParams)
        );
    }
}

#[test]
fn unsupported_chain_is_refused_4902_before_any_ui() {
    // Per-request chain (extension).
    let mut sut = boot();
    let ops = sut.dispatch(Arrive::extension("rid-4", "personal_sign", "[]", 999).event());
    let (code, kind, tid) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind, tid),
        (
            CODE_UNSUPPORTED_CHAIN,
            SignErrorKind::UnsupportedChain,
            EXT.to_owned()
        )
    );
    assert_eq!(
        sut.view().surface,
        SignSurface::Hidden,
        "never reached the sheet"
    );

    // Embedded chain (typed data domain).
    let params = r#"["0x0", "{\"domain\":{\"chainId\":999}}"]"#;
    let ops = sut.dispatch(Arrive::global("req-10", "eth_signTypedData_v4", params).event());
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (CODE_UNSUPPORTED_CHAIN, SignErrorKind::UnsupportedChain)
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn embedded_request_chain_switches_the_global_chain() {
    let mut sut = boot();
    let params = format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x0","chainId":"0x89"}}]"#);
    sut.dispatch(Arrive::global("req-11", "eth_sendTransaction", &params).event());
    let view = sut.view();
    assert_eq!(view.global_chain_id, 137);
    assert_eq!(view.request.expect("request").chain_id, 137);
}

#[test]
fn networks_unset_fails_closed() {
    let mut sut = Sut::new(); // no NetworksChanged
    sut.dispatch(Event::AccountsChanged {
        accounts: accounts(),
        active_index: 0,
    });
    let ops = sut.dispatch(Arrive::extension("rid-5", "personal_sign", "[]", 1).event());
    let (code, ..) = response_error(&ops[0]).expect("refusal");
    assert_eq!(code, CODE_UNSUPPORTED_CHAIN);
}

// ===========================================================================
// ⑦ §12.1.6 — granted-account reconcile before the approval surface
// ===========================================================================

#[test]
fn grant_mismatch_is_4100_never_a_silent_signer_swap() {
    let mut sut = boot();
    let mut arrive = Arrive::extension("rid-6", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.granted = Some(ACCT0.to_owned());
    arrive.requested = Some(ACCT1.to_owned());
    let ops = sut.dispatch(arrive.event());
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (CODE_UNAUTHORIZED, SignErrorKind::UnauthorizedAccount)
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn approval_surface_waits_for_the_account_switch_ack() {
    let mut sut = boot();
    let mut arrive = Arrive::extension("rid-7", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.granted = Some(ACCT1.to_owned()); // granted ≠ active
    let ops = sut.dispatch(arrive.event());
    assert!(
        matches!(ops.as_slice(), [Op::SwitchActiveAccount { index: 1 }]),
        "switch FIRST: {ops:?}"
    );
    let view = sut.view();
    assert!(view.reconcile_pending);
    assert!(!view.confirm_gate_open);

    // Approve before the ack must be inert.
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        ops.is_empty(),
        "the sheet may not act before the switch lands: {ops:?}"
    );

    let ops = sut.resolve(Res::AccountSwitched);
    assert!(ops.is_empty());
    assert!(sut.view().confirm_gate_open);

    // The signer is the GRANTED account.
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { address, credential_id, .. }]
            if address == ACCT1 && credential_id == "cred-1"),
        "signs from the granted account: {ops:?}"
    );
}

#[test]
fn unowned_grant_falls_back_to_the_visible_active_signer() {
    let mut sut = boot();
    let mut arrive = Arrive::extension("rid-8", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.granted = Some("0x9999999999999999999999999999999999999999".to_owned());
    let ops = sut.dispatch(arrive.event());
    assert!(ops.is_empty(), "no switch to perform: {ops:?}");
    let view = sut.view();
    assert!(!view.reconcile_pending);
    assert!(view.confirm_gate_open);
    assert_eq!(
        view.request.expect("request").signer_address.as_deref(),
        Some(ACCT0)
    );
}

// ===========================================================================
// ⑧ extension one-shot contract
// ===========================================================================

#[test]
fn a_settled_rid_never_signs_twice_replays_outcome() {
    let mut sut = boot();
    // Sign rid-9 to completion.
    sut.dispatch(Arrive::extension("rid-9", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: "0xsig".to_owned(),
        },
        now_ms: 9_000.0,
    });
    sut.resolve(Res::RecordPersisted);

    // The same rid arrives again (cold relaunch replay).
    let ops =
        sut.dispatch(Arrive::extension("rid-9", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    assert!(ops.is_empty(), "never a second sign: {ops:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden);
    assert_eq!(
        view.notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Submitted
        })
    );

    // A rejected rid replays 'rejected'.
    sut.dispatch(Arrive::extension("rid-10", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    sut.dispatch(Event::RejectTapped);
    sut.dispatch(Arrive::extension("rid-10", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    assert_eq!(
        sut.view().notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Rejected
        })
    );
}

#[test]
fn a_stale_request_payload_never_signs_and_never_responds() {
    let mut sut = boot();
    let mut arrive = Arrive::extension("rid-11", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.ts = Some(NOW - 301_000.0); // > 5 min old
    let ops = sut.dispatch(arrive.event());
    assert!(
        ops.is_empty(),
        "no sign, no response — the page recovers via 4900: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden);
    assert_eq!(view.notice, Some(SignNotice::Expired));

    // A fresh payload within the window signs normally.
    let mut arrive = Arrive::extension("rid-12", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.ts = Some(NOW - 299_000.0);
    sut.dispatch(arrive.event());
    assert_eq!(sut.view().surface, SignSurface::Sheet);
}

#[test]
fn only_an_explicit_reject_carries_4001() {
    // Funding cancel → recoverable -32603, never a durable 'rejected'.
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-12", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Denied { reason: None },
    });
    assert_eq!(sut.view().swipe_action, SignSwipeAction::FundingCancel);
    let ops = sut.dispatch(Event::SwipeDismissed);
    let (code, kind, _) = response_error(&ops[0]).expect("cancellation");
    assert_eq!(
        (code, kind),
        (CODE_INTERNAL, SignErrorKind::FundingCancelled)
    );

    // Passkey cancel → NO response at all; the modal stays open for a retry.
    sut.dispatch(Arrive::global("req-13", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::PasskeyCancelled,
        now_ms: 10_000.0,
    });
    assert!(ops.is_empty(), "cancel is never an error: {ops:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert!(view.confirm_gate_open, "the user may try again");
}

// ===========================================================================
// ⑨ paramsOverride (capped) flows through sign, submit and record
// ===========================================================================

#[test]
fn capped_override_is_what_gets_signed_submitted_and_recorded() {
    // Covered for submit + record in the funding/tx tests; assert the
    // signature-method path too.
    let capped = capped_approve_params();
    let mut sut = boot();
    sut.dispatch(
        Arrive::global("req-14", "eth_sendTransaction", &unlimited_approve_params()).event(),
    );
    sut.dispatch(approve(opts_with_override(&capped)));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { params_json, .. }] if params_json == &capped),
        "signed params are the CAPPED ones: {ops:?}"
    );
}

// ===========================================================================
// ⑩ EIP-5792 batches
// ===========================================================================

fn batch_params(calls_json: &str, capabilities_json: Option<&str>) -> String {
    match capabilities_json {
        Some(caps) => format!(r#"[{{"calls":{calls_json},"capabilities":{caps}}}]"#),
        None => format!(r#"[{{"calls":{calls_json}}}]"#),
    }
}

#[test]
fn batch_rejects_required_capability_5700_before_touching_the_wallet() {
    let mut sut = boot();
    let params = batch_params(
        &format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]"#),
        Some(r#"{"paymasterService":{}}"#),
    );
    sut.dispatch(Arrive::global("req-15", "wallet_sendCalls", &params).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert_eq!(
        ops.len(),
        1,
        "the refusal is the ONLY operation — no pre-check, no passkey: {ops:?}"
    );
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (
            CODE_UNSUPPORTED_CAPABILITY,
            SignErrorKind::UnsupportedCapability
        )
    );
}

#[test]
fn batch_with_optional_capability_proceeds() {
    let mut sut = boot();
    let params = batch_params(
        &format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]"#),
        Some(r#"{"flowControl":{"optional":true}}"#),
    );
    sut.dispatch(Arrive::global("req-16", "wallet_sendCalls", &params).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(
        matches!(ops.as_slice(), [Op::CheckBundlerFunding { .. }]),
        "{ops:?}"
    );
}

#[test]
fn empty_batch_is_refused() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-17", "wallet_sendCalls", &batch_params("[]", None)).event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!((code, kind), (CODE_INTERNAL, SignErrorKind::InvalidParams));
}

// ===========================================================================
// approval_guard composition — the unlimited submit chokepoint: refused
// unless the approval surface reported the person kept it as asked
// ===========================================================================

/// No consent (a shell that predates it, or a sheet that never mounted the
/// approval surface) — an unbounded amount is still refused.
#[test]
fn unlimited_single_approval_is_refused_at_the_submit_throat() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::global("req-18", "eth_sendTransaction", &unlimited_approve_params()).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default())); // no capped override
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert_eq!(ops.len(), 1, "refusal only — nothing is signed: {ops:?}");
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (CODE_INTERNAL, SignErrorKind::UnlimitedApproval)
    );
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::SignAndSubmit { .. })),
        "fail-closed: no SignAndSubmit ever issued"
    );
}

#[test]
fn unlimited_batch_leg_is_refused_per_leg() {
    let mut sut = boot();
    let calls = format!(
        r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1"}},{{"to":"{TOKEN}","data":"{}","value":"0x0"}}]"#,
        approve_calldata(&"f".repeat(64))
    );
    sut.dispatch(Arrive::global("req-19", "wallet_sendCalls", &batch_params(&calls, None)).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (CODE_INTERNAL, SignErrorKind::UnlimitedApproval)
    );
}

/// The approval surface showed it and the person kept it (2026-09-26): the
/// site's own bytes are signed — a Permit2 `approve(Permit2, MAX)` goes out
/// exactly as Uniswap built it.
#[test]
fn consented_unlimited_approval_submits_the_sites_own_bytes() {
    let mut sut = boot();
    let asked = unlimited_approve_params();
    sut.dispatch(Arrive::global("req-18b", "eth_sendTransaction", &asked).event());
    sut.dispatch(approve(SignApproveOpts {
        unlimited_approved: true,
        ..SignApproveOpts::default()
    }));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    let [Op::SignAndSubmit { params_json, .. }] = ops.as_slice() else {
        panic!("expected a submit: {ops:?}");
    };
    assert_eq!(params_json, &asked, "byte-identical to the request");
}

/// The EIP-5792 bundle whose next leg spends the allowance: kept as asked,
/// every leg goes out untouched, so the atomic batch does not revert.
#[test]
fn consented_unlimited_batch_leg_submits_the_bundle_untouched() {
    let mut sut = boot();
    let calls = format!(
        r#"[{{"to":"{TOKEN}","data":"{}","value":"0x0"}},{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]"#,
        approve_calldata(&"f".repeat(64))
    );
    let asked = batch_params(&calls, None);
    sut.dispatch(Arrive::global("req-19b", "wallet_sendCalls", &asked).event());
    sut.dispatch(approve(SignApproveOpts {
        unlimited_approved: true,
        ..SignApproveOpts::default()
    }));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    let [Op::SignAndSubmit { params_json, .. }] = ops.as_slice() else {
        panic!("expected a submit: {ops:?}");
    };
    assert_eq!(params_json, &asked);
}

/// The consent field is optional on the wire — an opts object from a shell
/// that predates it deserializes to `false`, never to a waiver.
#[test]
fn unlimited_consent_defaults_to_refusal_on_the_wire() {
    let opts: SignApproveOpts = serde_json::from_str(
        r#"{"max_fee_per_gas":null,"bundler_cost_wei":null,"gas_fee_token":null,"quoted_fee":null,"fee_collector":null,"params_override_json":null,"intent":null}"#,
    )
    .expect("pre-field opts still parse");
    assert!(!opts.unlimited_approved);
}

#[test]
fn capped_approval_passes_the_guard_and_submits() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-20", "eth_sendTransaction", &capped_approve_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
}

// ===========================================================================
// fee_policy composition — Tempo displayed-fee staleness at the submit side
// ===========================================================================

#[test]
fn stale_tempo_quote_is_rereviewed_never_silently_repriced() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-13", "eth_sendTransaction", &plain_send_params(), 4_217).event(),
    );

    let collector = "0x5555555555555555555555555555555555555555";
    let stale_opts = SignApproveOpts {
        quoted_fee: Some(SignQuotedFee {
            amount: "1".to_owned(), // below the $0.01 floor
            recipient: collector.to_owned(),
            tier: None,
        }),
        fee_collector: Some(collector.to_owned()),
        ..SignApproveOpts::default()
    };
    let ops = sut.dispatch(approve(stale_opts));
    assert!(ops.is_empty(), "a stale quote never submits: {ops:?}");
    let view = sut.view();
    assert_eq!(
        view.error.expect("stale").kind,
        SignErrorKind::StaleFeeQuote
    );
    assert_eq!(
        view.surface,
        SignSurface::Sheet,
        "stays reviewable for a re-quote"
    );

    // A recipient that no longer matches the collector is stale too.
    let floor = tempo_reimbursement(0, 0, TEMPO_FEE_TOKEN_DECIMALS).to_string();
    let moved_opts = SignApproveOpts {
        quoted_fee: Some(SignQuotedFee {
            amount: floor.clone(),
            recipient: "0x6666666666666666666666666666666666666666".to_owned(),
            tier: None,
        }),
        fee_collector: Some(collector.to_owned()),
        ..SignApproveOpts::default()
    };
    assert!(sut.dispatch(approve(moved_opts)).is_empty());

    // A fresh quote at the floor with the right recipient proceeds.
    let fresh_opts = SignApproveOpts {
        quoted_fee: Some(SignQuotedFee {
            amount: floor,
            recipient: collector.to_owned(),
            tier: None,
        }),
        fee_collector: Some(collector.to_owned()),
        ..SignApproveOpts::default()
    };
    let ops = sut.dispatch(approve(fresh_opts));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::CheckBundlerFunding {
                chain_id: 4_217,
                ..
            }]
        ),
        "{ops:?}"
    );
}

/// Spec 069: the speed the displayed fee was priced at travels to the
/// submission beside it, and the dead `rapid` never does.
#[test]
fn the_displayed_fee_names_its_tier_on_the_submission() {
    for (shown, named) in [
        (Some(FeeTier::Slow), Some(FeeTier::Slow)),
        (Some(FeeTier::Fast), Some(FeeTier::Fast)),
        (Some(FeeTier::Rapid), None),
        (None, None),
    ] {
        let mut sut = boot();
        sut.dispatch(
            Arrive::extension(
                "rid-tier",
                "eth_sendTransaction",
                &plain_send_params(),
                4_217,
            )
            .event(),
        );
        let collector = "0x5555555555555555555555555555555555555555";
        let floor = tempo_reimbursement(0, 0, TEMPO_FEE_TOKEN_DECIMALS).to_string();
        let ops = sut.dispatch(approve(SignApproveOpts {
            quoted_fee: Some(SignQuotedFee {
                amount: floor,
                recipient: collector.to_owned(),
                tier: shown,
            }),
            fee_collector: Some(collector.to_owned()),
            ..SignApproveOpts::default()
        }));
        assert!(
            matches!(ops.as_slice(), [Op::CheckBundlerFunding { .. }]),
            "{ops:?}"
        );
        let ops = sut.resolve(Res::PreCheck { funding: None });
        let submit = ops
            .iter()
            .find_map(|op| match op {
                Op::SignAndSubmit { quoted_fee, .. } => Some(quoted_fee.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("a submission: {ops:?}"));
        assert_eq!(submit.expect("quoted").tier, named, "{shown:?}");
    }
}

// ===========================================================================
// swipe routing, error terminality, transport drops, underfunded recovery
// ===========================================================================

#[test]
fn swipe_routes_by_phase() {
    let mut sut = boot();
    // Reviewing → reject (4001).
    sut.dispatch(Arrive::global("req-21", "personal_sign", r#"["0xdead","0x0"]"#).event());
    assert_eq!(sut.view().swipe_action, SignSwipeAction::Reject);
    let ops = sut.dispatch(Event::SwipeDismissed);
    let (code, kind, _) = response_error(&ops[0]).expect("reject");
    assert_eq!(
        (code, kind),
        (CODE_USER_REJECTED, SignErrorKind::UserRejected)
    );
    assert_eq!(sut.view().swipe_action, SignSwipeAction::None);
}

#[test]
fn no_second_response_after_a_terminal_error() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-22", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "boom".to_owned(),
            refused: false,
        },
        now_ms: 11_000.0,
    });
    assert!(response_error(&ops[0]).is_some(), "the one error response");

    // Neither a re-approve nor a reject may answer the same id again.
    assert!(sut.dispatch(approve(SignApproveOpts::default())).is_empty());
    assert!(
        sut.dispatch(Event::RejectTapped).is_empty(),
        "reject after response = dismiss"
    );
    assert_eq!(
        sut.view().surface,
        SignSurface::Hidden,
        "reject routed to dismiss"
    );
}

#[test]
fn transport_drop_clears_only_requests_it_owns() {
    // A global request dies with its durable transport…
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-23", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(Event::TransportDropped {
        transport_id: WP.to_owned(),
    });
    assert_eq!(sut.view().surface, SignSurface::Hidden);

    // …but a concurrent extension sign survives a WalletPair drop.
    sut.dispatch(Arrive::extension("rid-14", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    sut.dispatch(Event::TransportDropped {
        transport_id: WP.to_owned(),
    });
    assert_eq!(sut.view().request.expect("kept").id, "rid-14");
}

#[test]
fn underfunded_submit_heals_silently_then_offers_funding_and_retries() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-24", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });

    // Submit fails underfunded — silent sponsorship is FORCED first.
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Underfunded {
            message: "AA21 didn't pay prefund".to_owned(),
            funding: Some(funding_fixture()),
        },
        now_ms: 12_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::AttemptSponsorship { force: true, .. }]),
        "{ops:?}"
    );

    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Denied {
            reason: Some("cap reached".to_owned()),
        },
    });
    assert!(ops.is_empty());
    let view = sut.view();
    let funding = view.funding.expect("funding view");
    assert_eq!(funding.presentation, SignFundingPresentation::Topup);
    assert_eq!(funding.denial_reason.as_deref(), Some("cap reached"));

    // Continue → fresh pre-check → submit again, same request.
    let ops = sut.dispatch(Event::FundingCompleteTapped);
    assert!(matches!(
        ops.as_slice(),
        [Op::CheckBundlerFunding {
            bust_cache: true,
            ..
        }]
    ));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { id, .. }] if id == "req-24"));
}

#[test]
fn sponsorship_confirming_shows_the_confirming_beat() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-25", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Confirming,
    });
    let view = sut.view();
    assert_eq!(
        view.funding.expect("funding").presentation,
        SignFundingPresentation::Confirming
    );
}

#[test]
fn proactive_sponsorship_funded_proceeds_straight_to_submit() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-26", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Funded,
    });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
}

#[test]
fn batch_result_records_then_responds_with_the_batch_id() {
    // wallet_sendCalls has no onSubmitted — §4 takes the record-then-respond
    // path, and the record is the TS `dapp_tx` shape (batch id in the result).
    let mut sut = boot();
    let params = batch_params(
        &format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]"#),
        None,
    );
    sut.dispatch(Arrive::global("req-27", "wallet_sendCalls", &params).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: "0xbatchid".to_owned(),
        },
        now_ms: 13_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.kind == SignRecordKind::DappTx
                && record.record_id == "dapp-13000-tx"
                && record.result == "0xbatchid"),
        "{ops:?}"
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xbatchid".to_owned())))
    );
}

#[test]
fn malformed_params_fail_closed_at_approve() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-28", "eth_sendTransaction", "{not json").event());
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!((code, kind), (CODE_INTERNAL, SignErrorKind::InvalidParams));
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::SignAndSubmit { .. })),
        "nothing unparseable is ever signed"
    );
}

// ===========================================================================
// Spec 082 — a lost reply (RA3), the sheet's phase (RA9), the ending (RA8),
// the answer window (RA12) and a vanished asker (RB2)
// ===========================================================================

const LOCAL_OP: &str = "0x5538ce6978ab8924cce04d68c11cded19c36a8773e94656be6ada6315477ba1f";
const LANDED_TX: &str = "0xa6180e26c628e4ca1c3e474f5aa5d991ca9cba0fe4c28af9de40fc88a08b44f9";
const SUBMIT_BLOCK: u64 = 48_479_100;

/// Take a transaction request through approve and pre-check into the submit
/// stage, returning the machine.
fn submitting(id: &str) -> Sut {
    let mut sut = boot();
    sut.dispatch(Arrive::global(id, "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    sut
}

fn op_submitted_maybe(id: &str) -> Event {
    Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: 5_000.0,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
    }
}

/// T020: a submit whose reply was lost is recorded pending under the LOCAL
/// hash and handed to the tracker with the flag and the head — the same path
/// as an accepted op (RA3: no new outcome).
#[test]
fn a_maybe_sent_op_is_recorded_and_handed_over_under_the_local_hash() {
    let mut sut = submitting("req-m1");
    let ops = sut.dispatch(op_submitted_maybe("req-m1"));
    let [Op::PersistRecord { record }] = ops.as_slice() else {
        unreachable!("one pending record: {ops:?}")
    };
    assert_eq!(record.status, SignRecordStatus::Pending);
    assert_eq!(record.user_op_hash, LOCAL_OP);
    assert!(record.maybe_sent);
    assert_eq!(record.submit_block, Some(SUBMIT_BLOCK));

    let view = sut.view();
    assert_eq!(view.pending_op_hash.as_deref(), Some(LOCAL_OP));
    assert!(view.pending_op_maybe_sent);
    let handoff = view.tracker_handoff.expect("handoff");
    assert_eq!(handoff.user_op_hash, LOCAL_OP);
    assert!(handoff.maybe_sent);
    assert_eq!(handoff.submit_block, Some(SUBMIT_BLOCK));
    assert_eq!(handoff.record_ids, vec![record.record_id.clone()]);

    // The one answer the page gets: the op hash, Ok — never 4900 or -32603.
    assert!(sut.resolve(Res::RecordPersisted).is_empty());
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: LOCAL_OP.to_owned(),
        },
        now_ms: 125_000.0,
    });
    assert_eq!(ops.len(), 1, "{ops:?}");
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some(LOCAL_OP.to_owned())))
    );

    // The rid settled Submitted: a replay never signs twice.
    sut.dispatch(Arrive::global("req-m1", "eth_sendTransaction", &plain_send_params()).event());
    assert_eq!(
        sut.view().notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Submitted
        })
    );
}

/// T020: past the commitment point a swipe is a dismiss — no 4001 over an op
/// that may be on chain.
#[test]
fn a_swipe_after_a_maybe_sent_commit_is_a_dismiss() {
    let mut sut = submitting("req-m2");
    sut.dispatch(op_submitted_maybe("req-m2"));
    assert_eq!(sut.view().swipe_action, SignSwipeAction::Dismiss);
    let ops = sut.dispatch(Event::SwipeDismissed);
    assert!(ops.is_empty(), "no answer on a dismiss: {ops:?}");
    assert!(sut.dispatch(Event::RejectTapped).is_empty(), "no 4001");
}

/// T020: shells that predate 082 send none of the new fields.
#[test]
fn old_sign_json_without_the_new_fields_still_decodes() {
    let event: Option<Event> =
        serde_json::from_str(r#"{"type":"op_submitted","id":"r","user_op_hash":"0x1","now_ms":1}"#)
            .ok();
    assert!(matches!(
        event,
        Some(Event::OpSubmitted {
            maybe_sent: false,
            submit_block: None,
            ..
        })
    ));
    let record: Option<SignRecord> = serde_json::from_str(
        r#"{"record_id":"dapp-1-tx","kind":"dapp_tx","method":"eth_sendTransaction","params_json":"[]","result":"","from":"0x","chain_id":1,"now_ms":1,"status":"pending","user_op_hash":"0x1","dapp_origin":"o","intent":null}"#,
    )
    .ok();
    assert_eq!(
        record.map(|r| (r.maybe_sent, r.submit_block)),
        Some((false, None))
    );
    let handoff: Option<SignTrackerHandoff> =
        serde_json::from_str(r#"{"user_op_hash":"0x1","record_ids":[],"chain_id":1}"#).ok();
    assert_eq!(
        handoff.map(|h| (h.maybe_sent, h.submit_block)),
        Some((false, None))
    );
    let outcome: Option<SignSubmitOutcome> = serde_json::from_str(r#"{"type":"asker_gone"}"#).ok();
    assert_eq!(outcome, Some(SignSubmitOutcome::AskerGone));
}

/// T021 (RA9, G22): every row of the phase table. The network wait before the
/// passkey is Preparing — never AwaitingSignature.
#[test]
fn the_phase_follows_the_stage_and_the_prompt() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-p", "eth_sendTransaction", &plain_send_params()).event());
    assert_eq!(sut.view().phase, SignPhase::Idle, "the form");

    sut.dispatch(approve(SignApproveOpts::default()));
    assert_eq!(sut.view().phase, SignPhase::Preparing, "pre-check");
    // A prompt event during the pre-check is not accepted.
    assert!(sut
        .dispatch(Event::CeremonyStarted {
            id: "req-p".to_owned()
        })
        .is_empty());
    assert_eq!(sut.view().phase, SignPhase::Preparing);

    let ops = sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    assert!(matches!(ops.as_slice(), [Op::AttemptSponsorship { .. }]));
    assert_eq!(sut.view().phase, SignPhase::Preparing, "sponsoring");

    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Funded,
    });
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]));
    assert_eq!(
        sut.view().phase,
        SignPhase::Preparing,
        "submit stage, prompt not up yet"
    );

    // A stale id is dropped.
    assert!(sut
        .dispatch(Event::CeremonyStarted {
            id: "req-other".to_owned()
        })
        .is_empty());
    assert_eq!(sut.view().phase, SignPhase::Preparing);

    sut.dispatch(Event::CeremonyStarted {
        id: "req-p".to_owned(),
    });
    assert_eq!(sut.view().phase, SignPhase::AwaitingSignature);
    sut.dispatch(Event::CeremonyDone {
        id: "req-p".to_owned(),
    });
    assert_eq!(sut.view().phase, SignPhase::Submitting);
    // Forward only: a late "started" never re-opens the prompt.
    assert!(sut
        .dispatch(Event::CeremonyStarted {
            id: "req-p".to_owned()
        })
        .is_empty());
    assert_eq!(sut.view().phase, SignPhase::Submitting);

    // Reactive sponsorship after an underfunded submit: still Submitting.
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Underfunded {
            message: "AA21".to_owned(),
            funding: Some(funding_fixture()),
        },
        now_ms: 12_000.0,
    });
    assert!(matches!(
        ops.as_slice(),
        [Op::AttemptSponsorship { force: true, .. }]
    ));
    assert_eq!(sut.view().phase, SignPhase::Submitting);
}

/// T021: a message goes straight to the prompt; persisting its record is
/// Submitting; an accepted op implies the prompt is done.
#[test]
fn the_phase_of_a_message_and_of_an_accepted_op() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-q", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    assert_eq!(sut.view().phase, SignPhase::Preparing);
    sut.dispatch(Event::CeremonyStarted {
        id: "req-q".to_owned(),
    });
    assert_eq!(sut.view().phase, SignPhase::AwaitingSignature);
    let ops = sut.resolve(submit_ok("0xsig"));
    assert!(matches!(ops.as_slice(), [Op::PersistRecord { .. }]));
    assert_eq!(sut.view().phase, SignPhase::Submitting, "persisting");

    // A shell that never reports the prompt still reads Submitting once the
    // relay has the op.
    let mut sut = submitting("req-r");
    assert_eq!(sut.view().phase, SignPhase::Preparing);
    sut.dispatch(op_submitted_maybe("req-r"));
    assert_eq!(sut.view().phase, SignPhase::Submitting);
}

fn entry(status: TrackStatus, outcome: TrackOutcome, tx_hash: Option<&str>) -> TrackEntryView {
    TrackEntryView {
        user_op_hash: LOCAL_OP.to_owned(),
        chain_id: 100,
        record_ids: vec!["dapp-5000-tx".to_owned()],
        status,
        tx_hash: tx_hash.map(str::to_owned),
        polling: true,
        submitted_at_ms: Some(5_000.0),
        outcome,
        relay_tx_hash: None,
    }
}

fn ok_answer(result: &str) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(result.to_owned()),
    }
}

/// T022 (RA8 1): the ending an answer stands for.
#[test]
fn the_ending_of_an_answer() {
    assert_eq!(
        ending_of("personal_sign", &ok_answer("0xsig"), None),
        Some(SignEnding::Signed)
    );
    assert_eq!(
        ending_of("eth_signTypedData_v4", &ok_answer("0xsig"), Some(LOCAL_OP)),
        Some(SignEnding::Signed)
    );
    assert_eq!(
        ending_of("eth_sendTransaction", &ok_answer(LANDED_TX), Some(LOCAL_OP)),
        Some(SignEnding::Landed {
            tx_hash: LANDED_TX.to_owned(),
            user_op_hash: Some(LOCAL_OP.to_owned())
        })
    );
    assert_eq!(
        ending_of(
            "eth_sendTransaction",
            &ok_answer(&LOCAL_OP.to_uppercase().replace("0X", "0x")),
            Some(LOCAL_OP)
        ),
        Some(SignEnding::StillConfirming {
            user_op_hash: LOCAL_OP.to_owned()
        }),
        "the answer IS the op: the wait ran out"
    );
    assert_eq!(
        ending_of("wallet_sendCalls", &ok_answer(LOCAL_OP), Some(LOCAL_OP)),
        Some(SignEnding::StillConfirming {
            user_op_hash: LOCAL_OP.to_owned()
        })
    );
    assert_eq!(
        ending_of("eth_sendTransaction", &ok_answer(LANDED_TX), None),
        Some(SignEnding::Landed {
            tx_hash: LANDED_TX.to_owned(),
            user_op_hash: None
        })
    );
    let refused = SignResponsePayload::Err {
        code: CODE_USER_REJECTED,
        kind: SignErrorKind::UserRejected,
        message: None,
    };
    assert_eq!(ending_of("eth_sendTransaction", &refused, None), None);
    assert_eq!(
        ending_of(
            "eth_sendTransaction",
            &SignResponsePayload::Ok { result: None },
            None
        ),
        None
    );
    assert_eq!(
        ending_of("eth_sendTransaction", &ok_answer("  "), None),
        None
    );
}

/// T022 (RA8 2): every row of the data-model ending table — the tracker, not
/// the answer, decides "confirmed".
#[test]
fn the_ending_state_is_the_tracker_s() {
    let landed = SignEnding::Landed {
        tx_hash: LANDED_TX.to_owned(),
        user_op_hash: Some(LOCAL_OP.to_owned()),
    };
    let waiting = SignEnding::StillConfirming {
        user_op_hash: LOCAL_OP.to_owned(),
    };
    assert_eq!(
        ending_state(&SignEnding::Signed, None),
        SignEndingState::Signed
    );
    for ending in [&landed, &waiting] {
        assert_eq!(
            ending_state(
                ending,
                Some(&entry(
                    TrackStatus::Confirmed,
                    TrackOutcome::Final,
                    Some(LANDED_TX)
                ))
            ),
            SignEndingState::Confirmed {
                tx_hash: LANDED_TX.to_owned()
            }
        );
        assert_eq!(
            ending_state(
                ending,
                Some(&entry(
                    TrackStatus::Dropped,
                    TrackOutcome::Final,
                    Some(LANDED_TX)
                ))
            ),
            SignEndingState::Reverted {
                tx_hash: LANDED_TX.to_owned()
            }
        );
        // A landed answer never reads "not sent" / "refused" (round-2
        // review): `a_landed_ending_never_reads_not_sent_or_refused`.
        if ending == &waiting {
            assert_eq!(
                ending_state(
                    ending,
                    Some(&entry(TrackStatus::NotSent, TrackOutcome::Final, None))
                ),
                SignEndingState::NotSent
            );
            // Round 2 (RJ3): the relay refusing it is its own ending — no Retry.
            assert_eq!(
                ending_state(
                    ending,
                    Some(&entry(TrackStatus::Rejected, TrackOutcome::Final, None))
                ),
                SignEndingState::Refused
            );
        }
        for (status, outcome) in [
            (TrackStatus::Pending, TrackOutcome::Landing),
            (
                TrackStatus::AcceptedNotLanded,
                TrackOutcome::StillConfirming,
            ),
            (TrackStatus::Unreachable, TrackOutcome::Unknown),
        ] {
            assert_eq!(
                ending_state(ending, Some(&entry(status, outcome, None))),
                SignEndingState::Following {
                    user_op_hash: LOCAL_OP.to_owned(),
                    outcome,
                    fee_held: false
                },
                "{status:?}"
            );
        }
        assert_eq!(
            ending_state(
                ending,
                Some(&entry(
                    TrackStatus::FeeHeld,
                    TrackOutcome::StillConfirming,
                    None
                ))
            ),
            SignEndingState::Following {
                user_op_hash: LOCAL_OP.to_owned(),
                outcome: TrackOutcome::StillConfirming,
                fee_held: true
            }
        );
        // No entry yet — or another op's — is the ring, never "confirmed".
        assert_eq!(
            ending_state(ending, None),
            SignEndingState::Following {
                user_op_hash: LOCAL_OP.to_owned(),
                outcome: TrackOutcome::Landing,
                fee_held: false
            }
        );
        let mut other = entry(TrackStatus::Confirmed, TrackOutcome::Final, Some(LANDED_TX));
        other.user_op_hash = "0x01".to_owned();
        assert!(matches!(
            ending_state(ending, Some(&other)),
            SignEndingState::Following {
                outcome: TrackOutcome::Landing,
                ..
            }
        ));
    }
    // A may-have-been-sent op still following: the ring words for the op
    // hash answer; for a Landed answer the page already HAS its tx hash, so
    // the sheet never says "may have been sent" (DX6) — it is landing.
    assert_eq!(
        ending_state(
            &waiting,
            Some(&entry(TrackStatus::Pending, TrackOutcome::MaybeSent, None))
        ),
        SignEndingState::Following {
            user_op_hash: LOCAL_OP.to_owned(),
            outcome: TrackOutcome::MaybeSent,
            fee_held: false
        }
    );
    assert_eq!(
        ending_state(
            &landed,
            Some(&entry(TrackStatus::Pending, TrackOutcome::MaybeSent, None))
        ),
        SignEndingState::Following {
            user_op_hash: LOCAL_OP.to_owned(),
            outcome: TrackOutcome::Landing,
            fee_held: false
        }
    );
    // A reverted op whose tracker entry has no tx yet still links the answer's.
    assert_eq!(
        ending_state(
            &landed,
            Some(&entry(TrackStatus::Dropped, TrackOutcome::Final, None))
        ),
        SignEndingState::Reverted {
            tx_hash: LANDED_TX.to_owned()
        }
    );
}

/// T023 (ruling 9, W3): a receipt that reverted inside the wait is answered
/// its tx hash, sign_request writes no patch, and the tracker marks the SAME
/// record failed — which is what the sheet then draws.
#[test]
fn a_revert_inside_the_wait_answers_the_tx_hash_and_the_tracker_fails_the_record() {
    use vela_core::app::tx_tracker::{
        Event as TrackEvent, TrackOperation as TOp, TrackRecordPatch, TrackRecordStatus,
        TrackShellResult as TRes, TxTracker,
    };
    let mut sign = submitting("req-rv");
    sign.dispatch(Event::OpSubmitted {
        id: "req-rv".to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: 5_000.0,
        maybe_sent: false,
        submit_block: Some(SUBMIT_BLOCK),
    });
    sign.resolve(Res::RecordPersisted);
    let handoff = sign.view().tracker_handoff.expect("handoff");

    let ops = sign.resolve(submit_ok(LANDED_TX));
    assert_eq!(ops.len(), 1, "{ops:?}");
    let Op::SendResponse { payload, .. } = &ops[0] else {
        unreachable!("{ops:?}")
    };
    assert_eq!(payload, &ok_answer(LANDED_TX));

    let mut tracker = support::DomainDriver::<TxTracker>::new();
    tracker.dispatch(TrackEvent::Submitted {
        user_op_hash: handoff.user_op_hash.clone(),
        record_ids: handoff.record_ids.clone(),
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    });
    tracker.resolve(TRes::Clock { now_ms: NOW });
    let ops = tracker.resolve(TRes::ReceiptFailed {
        user_op_hash: LOCAL_OP.to_owned(),
        tx_hash: LANDED_TX.to_owned(),
        now_ms: NOW + 3_000.0,
    });
    assert_eq!(
        ops[0],
        TOp::UpdateTxRecords {
            ids: handoff.record_ids.clone(),
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Failed,
                tx_hash: None,
            },
        }
    );
    let view = tracker.view();
    let ending = ending_of("eth_sendTransaction", payload, Some(LOCAL_OP)).expect("ending");
    assert_eq!(
        ending_state(&ending, view.entries.first()),
        SignEndingState::Reverted {
            tx_hash: LANDED_TX.to_owned()
        }
    );
}

/// T024 (RA12): one 120 s answer window from the approve tap, never less
/// than 10 s of receipt wait.
#[test]
fn the_receipt_wait_is_what_is_left_of_the_answer_window() {
    assert_eq!(DAPP_TX_ANSWER_WINDOW_MS, 120_000.0);
    assert_eq!(dapp_receipt_wait_ms(0.0), 120_000.0);
    assert_eq!(dapp_receipt_wait_ms(46_000.0), 74_000.0);
    assert_eq!(dapp_receipt_wait_ms(115_000.0), 10_000.0);
    assert_eq!(dapp_receipt_wait_ms(200_000.0), 10_000.0);
    assert_eq!(dapp_receipt_wait_ms(-5.0), 120_000.0, "a clock step back");
    assert_eq!(EXTENSION_REQUEST_TTL_MS, 300_000.0, "unchanged");
}

/// T025 (RB2): the page left while the pre-check ran — the late pre-check
/// answer must not lead to a signature (the hole at `on_precheck`).
#[test]
fn a_transport_drop_during_the_precheck_stops_the_pipeline() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-d1", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    let ops = sut.dispatch(approve(SignApproveOpts::default()));
    assert!(matches!(ops.as_slice(), [Op::CheckBundlerFunding { .. }]));
    sut.dispatch(Event::TransportDropped {
        transport_id: EXT.to_owned(),
    });
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        ops.is_empty(),
        "nothing signs for a page that is gone: {ops:?}"
    );

    // During sponsoring too.
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-d2", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck {
        funding: Some(funding_fixture()),
    });
    sut.dispatch(Event::TransportDropped {
        transport_id: EXT.to_owned(),
    });
    let ops = sut.resolve(Res::Sponsorship {
        outcome: SignSponsorship::Funded,
    });
    assert!(ops.is_empty(), "{ops:?}");

    // Another transport's drop leaves a dedicated request's pipeline alone.
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-d3", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.dispatch(Event::TransportDropped {
        transport_id: WP.to_owned(),
    });
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
}

/// T025: past the passkey, a dropped transport changes nothing — the op may
/// be on its way, and its result is still recorded and delivered.
#[test]
fn a_transport_drop_after_the_passkey_lets_the_pipeline_finish() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-d4", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.dispatch(Event::CeremonyStarted {
        id: "rid-d4".to_owned(),
    });
    sut.dispatch(Event::TransportDropped {
        transport_id: EXT.to_owned(),
    });
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "rid-d4".to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: 5_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { .. }]),
        "{ops:?}"
    );
    sut.resolve(Res::RecordPersisted);
    let ops = sut.resolve(submit_ok(LANDED_TX));
    assert_eq!(
        response_ok(&ops[0]),
        Some((EXT.to_owned(), Some(LANDED_TX.to_owned())))
    );
}

/// T025: `AskerGone` — nothing sent, no answer, no record; the sheet clears
/// and the rid is not settled (it never signed).
#[test]
fn asker_gone_answers_nothing_and_records_nothing() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-g", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::AskerGone,
        now_ms: 6_000.0,
    });
    assert!(ops.is_empty(), "no Respond, no PersistRecord: {ops:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden);
    assert!(view.tracker_handoff.is_none());
    assert!(sut.outstanding().is_empty());
    sut.dispatch(
        Arrive::extension("rid-g", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    assert_eq!(sut.view().notice, None, "not settled");
}

/// Review of T020/T023 (RA2, RA8, G21): once `OpSubmitted` has handed the op
/// to the tracker it is on its way — or may be. Any later outcome that is not
/// a receipt or the op hash (a shell's receipt wait that threw, a stray
/// Underfunded, a passkey or asker event that can no longer be true) used to
/// answer the page -32603, show "submit failed" and patch the record failed
/// from here: the G21 double payment for a may-have-been-sent op (the dApp
/// reads -32603 as "not sent" and asks again) and a second writer racing the
/// tracker over an on-chain record. The page now gets exactly one answer, the
/// op hash (079's still-confirming contract), the rid settles Submitted, the
/// record is left to the tracker, and the sheet is not an error.
#[test]
fn after_the_hand_off_a_failure_is_answered_with_the_op_hash() {
    let failures = [
        SignSubmitOutcome::Failed {
            message: "All bundler endpoints failed".to_owned(),
            refused: false,
        },
        SignSubmitOutcome::Underfunded {
            message: "bundler underfunded".to_owned(),
            funding: None,
        },
        SignSubmitOutcome::PasskeyCancelled,
        SignSubmitOutcome::AskerGone,
    ];
    for (n, outcome) in failures.into_iter().enumerate() {
        let id = format!("req-h{n}");
        let mut sut = submitting(&id);
        sut.dispatch(op_submitted_maybe(&id));
        assert!(sut.resolve(Res::RecordPersisted).is_empty());
        let ops = sut.resolve(Res::Submit {
            outcome: outcome.clone(),
            now_ms: 60_000.0,
        });
        assert!(
            !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
            "{outcome:?}: the tracker alone closes the record: {ops:?}"
        );
        assert!(
            !ops.iter().any(|op| response_error(op).is_some()),
            "{outcome:?}: never -32603 over a sent op: {ops:?}"
        );
        assert_eq!(
            ops.iter().filter_map(response_ok).collect::<Vec<_>>(),
            vec![(WP.to_owned(), Some(LOCAL_OP.to_owned()))],
            "{outcome:?}: one Ok answer, the op hash"
        );
        let view = sut.view();
        assert!(
            view.error.is_none(),
            "{outcome:?}: no 'submit failed' sheet"
        );
        // The rid settled Submitted: a replay never signs twice.
        sut.dispatch(Arrive::global(&id, "eth_sendTransaction", &plain_send_params()).event());
        assert_eq!(
            sut.view().notice,
            Some(SignNotice::AlreadySettled {
                outcome: SignSettledOutcome::Submitted
            }),
            "{outcome:?}"
        );
    }
}

// ===========================================================================
// Spec 082 round 2 — the write-ahead record (T187, RJ1), and the answer that
// follows the tracker (T188, RJ3, RJ4)
// ===========================================================================

use vela_core::user_op::{NOT_SENT_DAPP_DETAIL, REFUSED_DAPP_DETAIL};

const RELAY_OP: &str = "0xa035b480c649967d6713388a017a91c8aea0c9732ed7f4ab51caba8a99199135";

fn op_signed(id: &str) -> Event {
    Event::OpSigned {
        id: id.to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        submit_block: Some(SUBMIT_BLOCK),
        now_ms: 4_000.0,
    }
}

fn op_submitted(id: &str, hash: &str, maybe_sent: bool) -> Event {
    Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: hash.to_owned(),
        now_ms: 5_000.0,
        maybe_sent,
        submit_block: Some(SUBMIT_BLOCK),
    }
}

fn op_tracked(status: TrackStatus, tx_hash: Option<&str>, now_ms: f64) -> Event {
    Event::OpTracked {
        user_op_hash: LOCAL_OP.to_owned(),
        status,
        tx_hash: tx_hash.map(str::to_owned),
        now_ms,
    }
}

fn err_detail(op: &Op) -> Option<(i32, SignErrorKind, Option<String>)> {
    match op {
        Op::SendResponse {
            payload:
                SignResponsePayload::Err {
                    code,
                    kind,
                    message,
                },
            ..
        } => Some((*code, *kind, message.clone())),
        _ => None,
    }
}

fn responses(ops: &[Op]) -> usize {
    ops.iter()
        .filter(|op| matches!(op, Op::SendResponse { .. }))
        .count()
}

/// Through the write-ahead: signed, the record written, cleared to POST.
/// Returns the record the write-ahead persisted.
fn written_ahead(sut: &mut Sut, id: &str) -> SignRecord {
    let ops = sut.dispatch(op_signed(id));
    let [Op::PersistRecord { record }] = ops.as_slice() else {
        unreachable!("the record first, and nothing else: {ops:?}")
    };
    let record = record.clone();
    let ops = sut.resolve_matching(is_persist, Res::RecordPersisted);
    assert_eq!(
        ops,
        vec![Op::ClearToPost {
            id: id.to_owned(),
            user_op_hash: LOCAL_OP.to_owned(),
        }],
        "cleared only once the record is on disk"
    );
    assert!(sut
        .resolve_matching(|op| matches!(op, Op::ClearToPost { .. }), Res::Responded)
        .is_empty());
    record
}

fn is_persist(op: &Op) -> bool {
    matches!(op, Op::PersistRecord { .. })
}

fn is_submit(op: &Op) -> bool {
    matches!(op, Op::SignAndSubmit { .. })
}

/// RJ1 (G34): the record precedes the clearance, and the clearance precedes
/// nothing else — the POST is the shell's, after it.
#[test]
fn the_record_precedes_clear_to_post_which_precedes_nothing_else() {
    let mut sut = submitting("req-w1");
    let record = written_ahead(&mut sut, "req-w1");
    assert_eq!(record.status, SignRecordStatus::Pending);
    assert_eq!(record.kind, SignRecordKind::DappTx);
    assert_eq!(record.user_op_hash, LOCAL_OP);
    assert!(record.maybe_sent, "nothing is known about the POST yet");
    assert_eq!(record.submit_block, Some(SUBMIT_BLOCK));
    let view = sut.view();
    let handoff = view
        .tracker_handoff
        .expect("the tracker holds it before the POST");
    assert_eq!(handoff.user_op_hash, LOCAL_OP);
    assert!(
        handoff.record_ids.is_empty(),
        "a POST is about to leave; its verdict names the record"
    );
    assert!(handoff.maybe_sent && !handoff.admitted);
    assert_eq!(
        view.phase,
        SignPhase::Submitting,
        "signed; going to the relay"
    );
    assert!(view.error.is_none() && view.tracker_withdraw.is_none());
    // Only the submit itself is still out.
    assert!(
        matches!(sut.outstanding().as_slice(), [Op::SignAndSubmit { .. }]),
        "{:?}",
        sut.outstanding()
    );
}

/// RJ1: a quit, crash or window close right after `OpSigned` leaves exactly
/// one pending "may have been sent" record the tracker resolves on the next
/// launch — and no answer.
#[test]
fn a_quit_after_op_signed_leaves_one_pending_maybe_sent_record() {
    let mut sut = submitting("req-w2");
    let ops = sut.dispatch(op_signed("req-w2"));
    let records: Vec<&SignRecord> = ops
        .iter()
        .filter_map(|op| match op {
            Op::PersistRecord { record } => Some(record),
            _ => None,
        })
        .collect();
    assert_eq!(records.len(), 1);
    assert!(records[0].maybe_sent && records[0].status == SignRecordStatus::Pending);
    assert_eq!(responses(&ops), 0);
}

/// A stale or wrong-id `OpSigned`, one before the submit stage, a second
/// one, and one after `OpSubmitted` are all dropped.
#[test]
fn a_stale_or_wrong_id_op_signed_is_dropped() {
    let mut sut = submitting("req-w3");
    assert!(sut.dispatch(op_signed("req-other")).is_empty(), "wrong id");
    written_ahead(&mut sut, "req-w3");
    assert!(sut.dispatch(op_signed("req-w3")).is_empty(), "twice");

    let mut early = boot();
    early.dispatch(Arrive::global("req-w4", "eth_sendTransaction", &plain_send_params()).event());
    early.dispatch(approve(SignApproveOpts::default()));
    assert!(early.dispatch(op_signed("req-w4")).is_empty(), "pre-check");

    let mut late = submitting("req-w5");
    late.dispatch(op_submitted("req-w5", LOCAL_OP, false));
    assert!(
        late.dispatch(op_signed("req-w5")).is_empty(),
        "after the POST"
    );
}

/// RJ1: the relay accepting the op patches the write-ahead record Admitted
/// (no second record) and hands the tracker `admitted`; a lost reply adds
/// nothing.
#[test]
fn accepted_patches_the_record_admitted() {
    let mut sut = submitting("req-w6");
    let record = written_ahead(&mut sut, "req-w6");
    let ops = sut.dispatch(op_submitted("req-w6", LOCAL_OP, false));
    assert_eq!(
        ops,
        vec![Op::UpdateRecord {
            record_id: record.record_id.clone(),
            close: SignRecordClose::Admitted,
        }]
    );
    let view = sut.view();
    let handoff = view.tracker_handoff.expect("handoff");
    assert!(handoff.admitted && !handoff.maybe_sent);
    assert_eq!(handoff.record_ids, vec![record.record_id.clone()]);
    assert_eq!(view.pending_op_hash.as_deref(), Some(LOCAL_OP));
    assert!(!view.pending_op_maybe_sent);

    let mut lost = submitting("req-w7");
    let record = written_ahead(&mut lost, "req-w7");
    let ops = lost.dispatch(op_submitted("req-w7", LOCAL_OP, true));
    assert!(ops.is_empty(), "the record already says so: {ops:?}");
    let view = lost.view();
    assert!(view.pending_op_maybe_sent);
    let handoff = view.tracker_handoff.expect("handoff");
    assert!(!handoff.admitted && handoff.maybe_sent);
    assert_eq!(handoff.record_ids, vec![record.record_id]);
}

/// RJ1: the relay answering with another hash withdraws the write-ahead
/// record and records the op under the relay's hash (today's path).
#[test]
fn another_relay_hash_withdraws_the_write_ahead() {
    let mut sut = submitting("req-w8");
    let record = written_ahead(&mut sut, "req-w8");
    let ops = sut.dispatch(op_submitted("req-w8", RELAY_OP, false));
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert_eq!(
        ops[0],
        Op::DeleteRecord {
            record_id: record.record_id.clone()
        }
    );
    let Op::PersistRecord { record: fresh } = &ops[1] else {
        unreachable!("{ops:?}")
    };
    assert_eq!(fresh.user_op_hash, RELAY_OP);
    let view = sut.view();
    assert_eq!(
        view.tracker_withdraw,
        Some(SignTrackerWithdraw {
            user_op_hash: LOCAL_OP.to_owned(),
            record_ids: vec![record.record_id],
        })
    );
    assert_eq!(
        view.tracker_handoff.map(|h| h.user_op_hash),
        Some(RELAY_OP.to_owned())
    );
    // Its ack clears nothing to POST again.
    assert!(sut
        .resolve_matching(is_persist, Res::RecordPersisted)
        .is_empty());
}

/// RJ1 + RJ3: a failure after `OpSigned` and before `OpSubmitted` proves
/// nothing was sent — the record is deleted, the tracker told to forget it,
/// and the page answered once with the not-sent sentence.
#[test]
fn not_sent_withdraws_the_record_and_answers_once() {
    let mut sut = submitting("req-w9");
    let record = written_ahead(&mut sut, "req-w9");
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: NOT_SENT_DAPP_DETAIL.to_owned(),
                refused: false,
            },
            now_ms: 9_000.0,
        },
    );
    assert_eq!(
        ops[0],
        Op::DeleteRecord {
            record_id: record.record_id.clone()
        }
    );
    assert_eq!(responses(&ops), 1, "{ops:?}");
    let answer = ops.iter().find_map(err_detail);
    assert_eq!(
        answer,
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(NOT_SENT_DAPP_DETAIL.to_owned())
        ))
    );
    assert!(!ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })));
    let view = sut.view();
    assert_eq!(
        view.tracker_withdraw,
        Some(SignTrackerWithdraw {
            user_op_hash: LOCAL_OP.to_owned(),
            record_ids: vec![record.record_id],
        })
    );
    assert!(view.tracker_handoff.is_none(), "never handed over again");
    assert!(view.error.is_some());
    assert!(!view.failure_refused);
}

/// RJ3: a submit-time refusal by the relay is answered with the refused
/// sentence (not the relay's words), and the sheet says "refused", no Retry.
#[test]
fn a_refused_submit_answers_the_refused_sentence() {
    let mut sut = submitting("req-w10");
    written_ahead(&mut sut, "req-w10");
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: "AA23 reverted".to_owned(),
                refused: true,
            },
            now_ms: 9_000.0,
        },
    );
    assert_eq!(responses(&ops), 1);
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned())
        ))
    );
    let view = sut.view();
    assert!(view.failure_refused);
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::SubmitFailed)
    );
    // The flag lives and dies with the error it qualifies: the next approve
    // clears both.
    let mut next = submitting("req-w11");
    next.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: "x".to_owned(),
                refused: true,
            },
            now_ms: 9_000.0,
        },
    );
    assert!(next.view().failure_refused);
    next.dispatch(Arrive::global("req-w12", "eth_sendTransaction", &plain_send_params()).event());
    next.dispatch(approve(SignApproveOpts::default()));
    let view = next.view();
    assert!(view.error.is_none() && !view.failure_refused);
}

/// An `AskerGone` after the write-ahead withdraws too, and answers nobody.
#[test]
fn asker_gone_after_op_signed_withdraws_and_answers_nobody() {
    let mut sut = submitting("req-w13");
    let record = written_ahead(&mut sut, "req-w13");
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::AskerGone,
            now_ms: 9_000.0,
        },
    );
    assert_eq!(
        ops,
        vec![Op::DeleteRecord {
            record_id: record.record_id
        }]
    );
    assert!(sut.view().tracker_withdraw.is_some());
}

/// DX-W3 (G36): accepted, then the relay's status says rejected at 15 s.
/// The page gets ONE answer — -32603, "refused, nothing was sent" — and the
/// window-end result that follows is dropped: no later Ok.
#[test]
fn a_rejection_the_tracker_learns_is_answered_refused_once() {
    let mut sut = submitting("req-t1");
    written_ahead(&mut sut, "req-t1");
    sut.dispatch(op_submitted("req-t1", LOCAL_OP, false));
    assert!(sut
        .dispatch(op_tracked(TrackStatus::Pending, None, 12_000.0))
        .is_empty());
    let ops = sut.dispatch(op_tracked(TrackStatus::Rejected, None, 20_000.0));
    assert_eq!(responses(&ops), 1, "{ops:?}");
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned())
        ))
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the tracker alone closes the record"
    );
    let view = sut.view();
    assert!(view.failure_refused);
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::SubmitFailed)
    );
    assert!(view.pending_op_hash.is_none(), "not 'submitting' any more");
    assert!(!view.confirm_gate_open);
    // The shell's own window end (the op hash) is dropped.
    let late = sut.resolve_matching(
        |op| matches!(op, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
            },
            now_ms: 125_000.0,
        },
    );
    assert!(late.is_empty(), "no later Ok: {late:?}");
    // The rid settled: never signed twice.
    sut.dispatch(Arrive::global("req-t1", "eth_sendTransaction", &plain_send_params()).event());
    assert_eq!(
        sut.view().notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Submitted
        })
    );
}

/// EX-S5 (G36): a lost reply the tracker proves never sent at 87 s — one
/// -32603 with the not-sent sentence, inside the window.
#[test]
fn a_not_sent_verdict_the_tracker_proves_is_answered_not_sent_once() {
    let mut sut = submitting("req-t2");
    written_ahead(&mut sut, "req-t2");
    sut.dispatch(op_submitted("req-t2", LOCAL_OP, true));
    let ops = sut.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0));
    assert_eq!(responses(&ops), 1, "{ops:?}");
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(NOT_SENT_DAPP_DETAIL.to_owned())
        ))
    );
    assert!(!sut.view().failure_refused);
    assert!(sut
        .dispatch(op_tracked(TrackStatus::NotSent, None, 90_000.0))
        .is_empty());
}

/// DX-W1 (G37): the chain check found the op at 72 s — the page gets the tx
/// hash at once, and the shell's window-end `ReceiptPending` is dropped.
#[test]
fn a_confirmation_the_tracker_finds_is_answered_at_once() {
    let mut sut = submitting("req-t3");
    written_ahead(&mut sut, "req-t3");
    sut.dispatch(op_submitted("req-t3", LOCAL_OP, true));
    assert!(
        sut.dispatch(op_tracked(TrackStatus::Confirmed, None, 70_000.0))
            .is_empty(),
        "no tx hash yet: wait"
    );
    let ops = sut.dispatch(op_tracked(
        TrackStatus::Confirmed,
        Some(LANDED_TX),
        72_000.0,
    ));
    assert_eq!(
        ops.iter().filter_map(response_ok).collect::<Vec<_>>(),
        vec![(WP.to_owned(), Some(LANDED_TX.to_owned()))]
    );
    assert_eq!(responses(&ops), 1);
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    let late = sut.resolve_matching(
        |op| matches!(op, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
            },
            now_ms: 120_000.0,
        },
    );
    assert!(late.is_empty(), "{late:?}");

    // A reverted op (ruling 9) is answered its tx hash too.
    let mut reverted = submitting("req-t4");
    reverted.dispatch(op_submitted("req-t4", LOCAL_OP, false));
    let ops = reverted.dispatch(op_tracked(TrackStatus::Dropped, Some(LANDED_TX), 30_000.0));
    assert_eq!(
        ops.iter().filter_map(response_ok).collect::<Vec<_>>(),
        vec![(WP.to_owned(), Some(LANDED_TX.to_owned()))]
    );
}

/// After the window answer (the op hash), a later confirmation changes
/// nothing: one answer per request.
#[test]
fn a_confirmation_after_the_window_answer_changes_nothing() {
    let mut sut = submitting("req-t5");
    sut.dispatch(op_submitted("req-t5", LOCAL_OP, false));
    sut.resolve_matching(is_persist, Res::RecordPersisted);
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
            },
            now_ms: 120_000.0,
        },
    );
    assert_eq!(responses(&ops), 1);
    assert!(sut
        .dispatch(op_tracked(
            TrackStatus::Confirmed,
            Some(LANDED_TX),
            130_000.0
        ))
        .is_empty());
    assert!(sut
        .dispatch(op_tracked(TrackStatus::Rejected, None, 131_000.0))
        .is_empty());
}

/// Another op's verdict, or one before `OpSubmitted`, answers nothing.
#[test]
fn only_this_op_s_verdict_after_its_post_answers() {
    let mut sut = submitting("req-t6");
    written_ahead(&mut sut, "req-t6");
    assert!(
        sut.dispatch(op_tracked(TrackStatus::Rejected, None, 6_000.0))
            .is_empty(),
        "before the POST's verdict"
    );
    sut.dispatch(op_submitted("req-t6", LOCAL_OP, false));
    let ops = sut.dispatch(Event::OpTracked {
        user_op_hash: RELAY_OP.to_owned(),
        status: TrackStatus::Confirmed,
        tx_hash: Some(LANDED_TX.to_owned()),
        now_ms: 20_000.0,
    });
    assert!(ops.is_empty(), "another op: {ops:?}");
}

/// Answered early, the pipeline's late `Submit` result can never answer a
/// NEWER request that has reached its own submit meanwhile.
#[test]
fn a_late_submit_of_an_answered_request_never_answers_a_newer_one() {
    let mut sut = submitting("req-a");
    sut.dispatch(op_submitted("req-a", LOCAL_OP, true));
    sut.dispatch(op_tracked(
        TrackStatus::Confirmed,
        Some(LANDED_TX),
        30_000.0,
    ));
    sut.dispatch(Arrive::global("req-b", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::CheckBundlerFunding { .. }),
        Res::PreCheck { funding: None },
    );
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { id, .. }] if id == "req-b"));
    let late = sut.resolve_matching(
        |op| matches!(op, Op::SignAndSubmit { id, .. } if id == "req-a"),
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
            },
            now_ms: 120_000.0,
        },
    );
    assert!(late.is_empty(), "req-a's late result: {late:?}");
    assert_eq!(
        sut.view().phase,
        SignPhase::Preparing,
        "req-b still in flight"
    );
}

/// The round-2 wire: old shells' JSON still reads, and the new words are
/// pinned.
#[test]
fn the_round_2_sign_wire() {
    let failed: Option<SignSubmitOutcome> =
        serde_json::from_str(r#"{"type":"failed","message":"x"}"#).ok();
    assert_eq!(
        failed,
        Some(SignSubmitOutcome::Failed {
            message: "x".to_owned(),
            refused: false
        })
    );
    let handoff: Option<SignTrackerHandoff> =
        serde_json::from_str(r#"{"user_op_hash":"0x1","record_ids":[],"chain_id":1}"#).ok();
    assert_eq!(handoff.map(|h| h.admitted), Some(false));
    assert_eq!(
        serde_json::to_value(SignRecordClose::Admitted).unwrap_or_default(),
        serde_json::json!({ "type": "admitted" })
    );
    assert_eq!(
        serde_json::to_value(Op::ClearToPost {
            id: "r".to_owned(),
            user_op_hash: "0x1".to_owned()
        })
        .unwrap_or_default(),
        serde_json::json!({ "type": "clear_to_post", "id": "r", "user_op_hash": "0x1" })
    );
    assert_eq!(
        serde_json::to_value(SignEndingState::Refused).unwrap_or_default(),
        serde_json::json!({ "type": "refused" })
    );
    let signed: Option<Event> = serde_json::from_str(
        r#"{"type":"op_signed","id":"r","user_op_hash":"0x1","submit_block":7,"now_ms":1}"#,
    )
    .ok();
    assert!(matches!(signed, Some(Event::OpSigned { .. })));
    let tracked: Option<Event> = serde_json::from_str(
        r#"{"type":"op_tracked","user_op_hash":"0x1","status":"confirmed","tx_hash":"0x2","now_ms":1}"#,
    )
    .ok();
    assert!(matches!(tracked, Some(Event::OpTracked { .. })));
}

// ===========================================================================
// Spec 082 round 2 — adversarial review of T187/T188
// ===========================================================================

/// The early tracker answer moves the attempt on (so the answered request's
/// late `Submit` result is dropped) — and a NEWER request whose account
/// switch is still out must not lose its ack to that move: the switch belongs
/// to the request that asked for it, never to a pipeline. Stranded, the newer
/// request stays `reconcile_pending` and can never be approved.
#[test]
fn an_early_tracker_answer_never_strands_a_newer_request_s_account_switch() {
    let mut sut = submitting("req-a");
    written_ahead(&mut sut, "req-a");
    sut.dispatch(op_submitted("req-a", LOCAL_OP, false));
    // A newer request, granted the other account, replaces the sheet; its
    // switch is out.
    let mut arrive = Arrive::extension("req-b", "personal_sign", r#"["0xdead","0x0"]"#, 1);
    arrive.granted = Some(ACCT1.to_owned());
    let ops = sut.dispatch(arrive.event());
    assert!(
        matches!(ops.as_slice(), [Op::SwitchActiveAccount { index: 1 }]),
        "{ops:?}"
    );
    // req-a's tracker verdict lands before the switch acks.
    let ops = sut.dispatch(op_tracked(
        TrackStatus::Confirmed,
        Some(LANDED_TX),
        30_000.0,
    ));
    assert_eq!(responses(&ops), 1, "{ops:?}");
    sut.resolve_matching(
        |op| matches!(op, Op::SwitchActiveAccount { .. }),
        Res::AccountSwitched,
    );
    let view = sut.view();
    assert!(!view.reconcile_pending, "req-b's switch landed");
    assert!(view.confirm_gate_open, "req-b can be approved");
}

/// The tracker's `NotSent` is a verdict on an op the relay never showed it
/// holds. For an op the relay ACCEPTED it can only be stale — reached while
/// the POST was still out, before the tracker learned `admitted` — and
/// answering it would tell the page "nothing was sent" over an op on its way
/// (the page asks again: the double payment). It waits for the tracker.
#[test]
fn a_not_sent_verdict_never_answers_an_op_the_relay_accepted() {
    let mut sut = submitting("req-ns");
    written_ahead(&mut sut, "req-ns");
    sut.dispatch(op_submitted("req-ns", LOCAL_OP, false));
    let ops = sut.dispatch(op_tracked(TrackStatus::NotSent, None, 75_000.0));
    assert!(ops.is_empty(), "no -32603 over an accepted op: {ops:?}");
    assert!(sut.view().error.is_none());
    // The tracker's real verdict still answers.
    let ops = sut.dispatch(op_tracked(
        TrackStatus::Confirmed,
        Some(LANDED_TX),
        90_000.0,
    ));
    assert_eq!(
        ops.iter().filter_map(response_ok).collect::<Vec<_>>(),
        vec![(WP.to_owned(), Some(LANDED_TX.to_owned()))]
    );
    // A may-have-been-sent op is still answered "not sent" (EX-S5).
    let mut maybe = submitting("req-ns2");
    written_ahead(&mut maybe, "req-ns2");
    maybe.dispatch(op_submitted("req-ns2", LOCAL_OP, true));
    let ops = maybe.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0));
    assert_eq!(responses(&ops), 1, "{ops:?}");
}

/// A `Landed` ending holds a tx hash — a receipt the shell saw, or the
/// tracker's own verdict. The tracker's `NotSent` / `Rejected` against it can
/// only be stale (reached while the POST was still out, before the tracker
/// learned the relay took it), and drawn, it told the person "nothing was
/// sent" — send it again — over money that moved. It follows the tracker to
/// its real verdict instead. An op-hash answer (`StillConfirming`) keeps the
/// tracker's word: nothing proves it landed.
#[test]
fn a_landed_ending_never_reads_not_sent_or_refused() {
    let landed = SignEnding::Landed {
        tx_hash: LANDED_TX.to_owned(),
        user_op_hash: Some(LOCAL_OP.to_owned()),
    };
    for status in [TrackStatus::NotSent, TrackStatus::Rejected] {
        assert_eq!(
            ending_state(&landed, Some(&entry(status, TrackOutcome::Final, None))),
            SignEndingState::Following {
                user_op_hash: LOCAL_OP.to_owned(),
                outcome: TrackOutcome::Landing,
                fee_held: false
            },
            "{status:?}"
        );
    }
    let waiting = SignEnding::StillConfirming {
        user_op_hash: LOCAL_OP.to_owned(),
    };
    assert_eq!(
        ending_state(
            &waiting,
            Some(&entry(TrackStatus::NotSent, TrackOutcome::Final, None))
        ),
        SignEndingState::NotSent
    );
    assert_eq!(
        ending_state(
            &waiting,
            Some(&entry(TrackStatus::Rejected, TrackOutcome::Final, None))
        ),
        SignEndingState::Refused
    );
}

/// `RecordPersisted` names no record. A write-ahead withdrawn before its
/// persist was acked (the shell's `WRITE_AHEAD_WAIT_MS` ran out on a stalled
/// disk) leaves that ack in flight; arriving while a NEWER request's own
/// write-ahead waits, it cleared that request to POST before its record was
/// on disk — the RJ1 promise broken for the newer op. The late ack is the
/// withdrawn pipeline's and is dropped.
#[test]
fn a_withdrawn_write_ahead_s_late_ack_never_clears_a_newer_post() {
    let mut sut = submitting("req-old");
    let ops = sut.dispatch(op_signed("req-old"));
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { .. }]),
        "{ops:?}"
    );
    // No clearance in time: the shell gives up without POSTing.
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: NOT_SENT_DAPP_DETAIL.to_owned(),
                refused: false,
            },
            now_ms: 9_500.0,
        },
    );
    assert!(
        ops.iter().any(|op| matches!(op, Op::DeleteRecord { .. })),
        "{ops:?}"
    );
    // A newer request reaches its own write-ahead.
    sut.dispatch(Arrive::global("req-new", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::CheckBundlerFunding { .. }),
        Res::PreCheck { funding: None },
    );
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { id, .. }] if id == "req-new"));
    let ops = sut.dispatch(op_signed("req-new"));
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { .. }]),
        "{ops:?}"
    );
    // The OLD record's ack lands first: nothing is cleared.
    let late = sut.resolve_matching(is_persist, Res::RecordPersisted);
    assert!(
        late.is_empty(),
        "the old ack cleared the new POST: {late:?}"
    );
    // The new record's own ack clears it.
    let ops = sut.resolve_matching(is_persist, Res::RecordPersisted);
    assert_eq!(
        ops,
        vec![Op::ClearToPost {
            id: "req-new".to_owned(),
            user_op_hash: LOCAL_OP.to_owned(),
        }]
    );
}

/// Moving the attempt on for the withdrawn write-ahead's late ack drops that
/// ack only: the reactive sponsorship an underfunded verdict starts is issued
/// under the new attempt, and its answer still opens the funding view.
#[test]
fn a_withdrawn_write_ahead_s_bump_keeps_the_reactive_sponsorship() {
    let mut sut = submitting("req-u");
    sut.dispatch(op_signed("req-u"));
    // The wait ran out before the ack: nothing was POSTed.
    let ops = sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Underfunded {
                message: "AA21 didn't pay prefund".to_owned(),
                funding: Some(funding_fixture()),
            },
            now_ms: 9_500.0,
        },
    );
    assert!(
        ops.iter().any(|op| matches!(op, Op::DeleteRecord { .. })),
        "{ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::AttemptSponsorship { force: true, .. })),
        "{ops:?}"
    );
    // The withdrawn record's ack is dropped...
    assert!(sut
        .resolve_matching(is_persist, Res::RecordPersisted)
        .is_empty());
    // ...and the sponsorship's answer is not.
    sut.resolve_matching(
        |op| matches!(op, Op::AttemptSponsorship { .. }),
        Res::Sponsorship {
            outcome: SignSponsorship::Denied { reason: None },
        },
    );
    let funding = sut.view().funding.expect("the funding view");
    assert_eq!(funding.presentation, SignFundingPresentation::Topup);
}

// ===========================================================================
// Spec 082 round 2 — second review: the tracker's grace counts from the
// POST's verdict
// ===========================================================================

fn handoff_of(sut: &Sut) -> SignTrackerHandoff {
    sut.view().tracker_handoff.expect("a hand-off")
}

/// The write-ahead hands the tracker the op before its POST with NO record:
/// the tracker holds a may-have-been-sent op whose bytes have not left, and
/// must not judge "not sent" from a relay that cannot have seen it yet. The
/// POST's verdict names the record.
#[test]
fn the_write_ahead_hands_the_op_over_with_no_record() {
    let mut sut = submitting("req-g1");
    sut.dispatch(op_signed("req-g1"));
    let handoff = handoff_of(&sut);
    assert_eq!(handoff.user_op_hash, LOCAL_OP);
    assert!(handoff.record_ids.is_empty(), "{handoff:?}");
    assert!(handoff.maybe_sent && !handoff.admitted);
    assert_eq!(handoff.submit_block, Some(SUBMIT_BLOCK));
}

/// A "may have been sent" verdict hands the record over — a hand-off every
/// shell feeds (its de-duplication key differs from the write-ahead's).
#[test]
fn a_maybe_sent_verdict_hands_the_record_over() {
    let mut sut = submitting("req-g2");
    let record = written_ahead(&mut sut, "req-g2");
    let ops = sut.dispatch(op_submitted("req-g2", LOCAL_OP, true));
    assert!(ops.is_empty(), "no second record: {ops:?}");
    assert_eq!(
        handoff_of(&sut),
        SignTrackerHandoff {
            user_op_hash: LOCAL_OP.to_owned(),
            record_ids: vec![record.record_id],
            chain_id: 1,
            maybe_sent: true,
            submit_block: Some(SUBMIT_BLOCK),
            admitted: false,
        }
    );
}

/// A result standing for the written-ahead op, with no `OpSubmitted` before
/// it, adopts the record — and hands it over: taken with a receipt
/// (admitted), or the window's end with the local hash (may have been sent).
#[test]
fn an_adopted_write_ahead_hands_its_record_over() {
    let mut landed = submitting("req-g3");
    let record = written_ahead(&mut landed, "req-g3");
    let ops = landed.resolve_matching(is_submit, submit_ok(LANDED_TX));
    assert_eq!(responses(&ops), 1, "{ops:?}");
    let handoff = handoff_of(&landed);
    assert_eq!(handoff.record_ids, vec![record.record_id]);
    assert!(handoff.admitted && !handoff.maybe_sent, "{handoff:?}");

    let mut pending = submitting("req-g4");
    let record = written_ahead(&mut pending, "req-g4");
    let ops = pending.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
            },
            now_ms: 120_000.0,
        },
    );
    assert_eq!(responses(&ops), 1, "{ops:?}");
    let handoff = handoff_of(&pending);
    assert_eq!(handoff.record_ids, vec![record.record_id]);
    assert!(handoff.maybe_sent && !handoff.admitted, "{handoff:?}");
}
