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
    method_kind, request_names_other_chain, required_capabilities, reverted_detail,
    reverted_transaction, sign_account_index, Event, SignAccountRef, SignApproveOpts,
    SignDappIdentity, SignEnding, SignEndingState, SignErrorKind, SignFundingNeeded,
    SignFundingPresentation, SignMethodKind, SignNotice, SignOperation as Op, SignPhase,
    SignQuotedFee, SignRecord, SignRecordClose, SignRecordKind, SignRecordStatus, SignRequest,
    SignResponsePayload, SignSettledOutcome, SignShellResult as Res, SignSponsorship,
    SignSubmitOutcome, SignSurface, SignSwipeAction, SignTrackerHandoff, SignTrackerWithdraw,
    CHAIN_MISMATCH_MESSAGE, CODE_INTERNAL, CODE_INVALID_PARAMS, CODE_UNAUTHORIZED,
    CODE_UNSUPPORTED_CAPABILITY, CODE_UNSUPPORTED_CHAIN, CODE_USER_REJECTED,
    DAPP_TX_ANSWER_WINDOW_MS, EXTENSION_REQUEST_TTL_MS, NOT_CONFIRMED_MESSAGE, PAGE_WAIT_CAP_MS,
    REVERTED_MESSAGE,
};
use vela_core::app::token_trust::TrustSimJudgment;
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
    first_party: bool,
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
            first_party: false,
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
            first_party: false,
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
            first_party: self.first_party,
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
        } => Some((transport_id.clone(), result.as_ref().map(page_text))),
        _ => None,
    }
}

/// The page's `result` as text: a string as it is, anything else (a batch's
/// `{ id }`, spec 097 G) as its JSON — so a shape a test did not expect never
/// reads as a bare hash.
fn page_text(result: &serde_json::Value) -> String {
    result
        .as_str()
        .map_or_else(|| result.to_string(), str::to_owned)
}

/// The page was told its transaction is not confirmed yet (083, owner ruling
/// 2026-10-01): `-32603`, [`NOT_CONFIRMED_MESSAGE`] naming `op` — never `op`
/// as if it were a transaction hash.
fn not_confirmed(op: &Op, user_op: &str) -> bool {
    matches!(op, Op::SendResponse {
        payload: SignResponsePayload::Err { code: CODE_INTERNAL, message: Some(message), .. },
        ..
    } if *message == format!("{NOT_CONFIRMED_MESSAGE} (user operation {user_op})"))
}

/// The answer in `ops`, as [`ending_of`] reads it for the sheet.
fn ending_in(ops: &[Op], method: &str, user_op: Option<&str>) -> Option<SignEnding> {
    ops.iter().find_map(|op| match op {
        Op::SendResponse { payload, .. } => ending_of(method, payload, user_op),
        _ => None,
    })
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
    // _v4 order: [address, typedData]; hex string chain. Read as the ONE
    // document (audit 2026-10-01): a real account, an EIP-712 document.
    let doc = |chain: serde_json::Value| {
        serde_json::json!({"types": {"EIP712Domain": []}, "primaryType": "Mail",
                           "domain": {"chainId": chain}, "message": {}})
    };
    let typed = serde_json::json!([ACCT0, doc(serde_json::json!("0x89")).to_string()]);
    assert_eq!(
        extract_request_chain_id("eth_signTypedData_v4", &typed),
        Some(137)
    );
    // unsuffixed order: [typedData, address]; numeric chain.
    let typed_v1 = serde_json::json!([doc(serde_json::json!(137)), ACCT0]);
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
    // Record FIRST — no response yet. The record keeps that a signature was
    // given, never the signature itself (spec 093): that is the page's.
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.record_id == "dapp-1000-msg"
                && record.kind == SignRecordKind::SignMessage
                && record.status == SignRecordStatus::Confirmed
                && record.result.is_empty()),
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

/// Issue 262: the receipt wait timed out. The page is told the transaction is
/// not confirmed yet — never the op hash (083, owner ruling 2026-10-01) — and
/// the record stays PENDING — no confirming patch — and the tracker (which
/// already holds the op) is the only one who may close it.
#[test]
fn receipt_timeout_answers_not_confirmed_and_leaves_the_record_pending() {
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
    assert!(not_confirmed(&ops[0], "0xophash"), "{ops:?}");
    assert_eq!(
        ending_in(&ops, "eth_sendTransaction", Some("0xophash")),
        Some(SignEnding::StillConfirming {
            user_op_hash: "0xophash".to_owned()
        }),
        "the sheet follows the op"
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
    assert!(not_confirmed(&ops[0], "0xophash"), "{ops:?}");
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
        sender: None,
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
                settlement: None,
            },
        }),
        "the tracker closes the SAME record with the real tx hash: {ops:?}"
    );
}

/// Spec 082 RA8 (was: sign_request patched the record failed and answered
/// -32603 — "no eternal pending"). Past `OpSubmitted` the tracker holds the
/// record and alone closes it: a dropped op's failed receipt patches it
/// failed there, so nothing is left pending forever. The page is told it is
/// not confirmed yet (083) — never "failed", which it would retry over an op
/// the relay accepted, and never the op hash.
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
            signer: None,
        },
        now_ms: 7_000.0,
    });
    assert_eq!(ops.len(), 1, "no record patch from here: {ops:?}");
    assert!(not_confirmed(&ops[0], "0xop"), "{ops:?}");
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
    // Beside the name the dApp gave, the origin it came from — the only one
    // of the two Activity may call a site (083 H2 review).
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }] if record.dapp_url == ORIGIN),
        "{ops:?}"
    );
}

/// The wallet's own request (the passkey-registry backup) is first-party
/// only because the shell that raised it says so — the view passes the flag
/// through, a page's request never carries it, and a shell that predates the
/// field (no `first_party` on the wire) gets a third-party sheet.
#[test]
fn the_wallets_own_request_is_first_party_and_a_pages_never_is() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("rid-page", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let request = sut.view().request.expect("request");
    assert!(
        !request.first_party,
        "a page's request is never first-party"
    );
    sut.dispatch(Event::RejectTapped);

    let mut own = Arrive::global("rid-own", "eth_sendTransaction", &plain_send_params());
    own.transport_id = "wallet".to_owned();
    own.first_party = true;
    sut.dispatch(own.event());
    let request = sut.view().request.expect("request");
    assert_eq!(request.id, "rid-own");
    assert!(
        request.first_party,
        "the flag the shell set reaches the sheet"
    );

    // A newer request taking the sheet does not inherit it.
    sut.dispatch(Event::RejectTapped);
    sut.dispatch(Arrive::global("rid-next", "personal_sign", r#"["0xbeef","0x0"]"#).event());
    let request = sut.view().request.expect("request");
    assert_eq!(request.id, "rid-next");
    assert!(!request.first_party, "first-party belongs to one request");

    // On the wire: absent means false; present is read as written.
    let wire = |extra: &str| {
        format!(
            r#"{{"type":"request_arrived","id":"w","method":"personal_sign","params_json":"[]","origin":"{ORIGIN}","transport_id":"{WP}","dedicated_transport":false,"per_request_chain":null,"dapp":null,"granted_address":null,"requested_address":null,"request_ts_ms":null,"now_ms":1{extra}}}"#
        )
    };
    for (extra, expected) in [
        ("", false),
        (r#","first_party":false"#, false),
        (r#","first_party":true"#, true),
    ] {
        let event: Event = serde_json::from_str(&wire(extra)).expect("a request_arrived event");
        match event {
            Event::RequestArrived { first_party, .. } => {
                assert_eq!(first_party, expected, "first_party from {extra:?}")
            }
            other => panic!("parsed as {other:?}"),
        }
    }
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

    // Embedded chain (typed data domain) — a well-formed request, so the
    // chain is what refuses it.
    let doc = serde_json::json!({
        "types": {"EIP712Domain": [{"name": "chainId", "type": "uint256"}],
                  "Mail": [{"name": "contents", "type": "string"}]},
        "primaryType": "Mail", "domain": {"chainId": 999}, "message": {"contents": "hi"}
    });
    let params = serde_json::json!([ACCT0, doc.to_string()]).to_string();
    let ops = sut.dispatch(Arrive::global("req-10", "eth_signTypedData_v4", &params).event());
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

// ---------------------------------------------------------------------------
// 089 — a request naming another chain than the site's is refused
// ---------------------------------------------------------------------------

/// The refusal 089 answers: -32602, `invalid_params`, the constant sentence,
/// to the request's own transport.
fn chain_mismatch_refusal(op: &Op) -> bool {
    matches!(op, Op::SendResponse {
        transport_id,
        payload: SignResponsePayload::Err {
            code: CODE_INVALID_PARAMS,
            kind: SignErrorKind::InvalidParams,
            message: Some(message),
        },
        ..
    } if transport_id == EXT && message == CHAIN_MISMATCH_MESSAGE)
}

fn tx_on(chain: &str) -> String {
    format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1","chainId":{chain}}}]"#)
}

fn batch_on(chain: &str) -> String {
    format!(
        r#"[{{"version":"2.0.0","chainId":{chain},"calls":[{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]}}]"#
    )
}

fn typed_on(chain: serde_json::Value) -> String {
    let doc = serde_json::json!({
        "types": {"EIP712Domain": [{"name": "chainId", "type": "uint256"}],
                  "Mail": [{"name": "contents", "type": "string"}]},
        "primaryType": "Mail", "domain": {"chainId": chain}, "message": {"contents": "hi"}
    });
    serde_json::json!([ACCT0, doc.to_string()]).to_string()
}

/// Arrive stamped `stamp`, expect the 089 refusal as the ONLY operation, and
/// no sheet — nothing shown, nothing signable, the global chain untouched.
fn assert_refused_before_any_sheet(method: &str, params: &str, stamp: u32) {
    let mut sut = boot();
    let ops = sut.dispatch(Arrive::extension("rid-89", method, params, stamp).event());
    assert!(
        matches!(ops.as_slice(), [op] if chain_mismatch_refusal(op)),
        "{method} {params} stamped {stamp}: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Hidden, "{method}: never a sheet");
    assert!(view.request.is_none(), "{method}: nothing pending");
    assert_eq!(
        view.global_chain_id, 1,
        "{method}: the global chain untouched"
    );
    assert!(
        sut.dispatch(approve(SignApproveOpts::default())).is_empty(),
        "{method}: nothing left to approve"
    );
}

#[test]
fn stamped_chain_1_refuses_a_transaction_for_gnosis_before_any_sheet() {
    // The reported case: the site is on Ethereum in the wallet, the dApp
    // prepared its call for Gnosis. Refused whether or not the wallet knows
    // the named chain — it is the mismatch that refuses, not the chain.
    assert_refused_before_any_sheet("eth_sendTransaction", &tx_on(r#""0x64""#), 1);
    assert_refused_before_any_sheet("eth_sendTransaction", &tx_on("100"), 1);
    assert_refused_before_any_sheet("eth_sendTransaction", &tx_on(r#""0x89""#), 1);
}

#[test]
fn stamped_chain_1_refuses_a_batch_for_gnosis_before_any_sheet() {
    assert_refused_before_any_sheet("wallet_sendCalls", &batch_on(r#""0x64""#), 1);
    assert_refused_before_any_sheet("wallet_sendCalls", &batch_on(r#""0x89""#), 1);
}

#[test]
fn stamped_chain_refuses_typed_data_whose_domain_names_another_chain() {
    // The Safe's signature is bound to the chain it is signed on; a domain on
    // another chain would get a signature for neither. MetaMask refuses it
    // too; the in-app path switches to the domain's chain instead.
    assert_refused_before_any_sheet("eth_signTypedData_v4", &typed_on(serde_json::json!(137)), 1);
    assert_refused_before_any_sheet(
        "eth_signTypedData_v4",
        &typed_on(serde_json::json!("0x64")),
        1,
    );
}

#[test]
fn stamped_chain_refuses_a_chain_id_that_reads_as_no_chain() {
    // A chainId the wallet cannot read is not "no chain": it was meant to be
    // SOME chain, and there is no telling it is this one.
    for written in [
        r#""0x""#,
        r#""gnosis""#,
        "0",
        r#""0x0""#,
        r#""1x""#,
        "1.5",
        "{}",
    ] {
        assert_refused_before_any_sheet("eth_sendTransaction", &tx_on(written), 1);
    }
}

#[test]
fn stamped_chain_matching_the_request_proceeds_on_it() {
    for (method, params) in [
        ("eth_sendTransaction", tx_on(r#""0x89""#)),
        ("eth_sendTransaction", tx_on("137")),
        ("eth_sendTransaction", tx_on(r#""137""#)),
        ("wallet_sendCalls", batch_on(r#""0x89""#)),
    ] {
        let mut sut = boot();
        sut.dispatch(Arrive::extension("rid-89", method, &params, 137).event());
        let view = sut.view();
        assert_eq!(view.surface, SignSurface::Sheet, "{method} {params}");
        assert_eq!(view.request.expect("request").chain_id, 137);
        assert_eq!(
            view.global_chain_id, 1,
            "a stamp never moves the global chain"
        );
        let ops = sut.dispatch(approve(SignApproveOpts::default()));
        assert!(
            matches!(
                ops.as_slice(),
                [Op::CheckBundlerFunding { chain_id: 137, .. }]
            ),
            "{method}: {ops:?}"
        );
        let ops = sut.resolve(Res::PreCheck { funding: None });
        assert!(
            matches!(ops.as_slice(), [Op::SignAndSubmit { chain_id: 137, .. }]),
            "{method}: {ops:?}"
        );
    }

    // Typed data whose domain is the stamped chain reaches the sheet.
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension(
            "rid-89",
            "eth_signTypedData_v4",
            &typed_on(serde_json::json!("0x89")),
            137,
        )
        .event(),
    );
    assert_eq!(sut.view().surface, SignSurface::Sheet);
}

#[test]
fn stamped_chain_with_no_chain_in_the_request_proceeds_on_the_stamp() {
    for (method, params) in [
        ("eth_sendTransaction", plain_send_params()),
        ("eth_sendTransaction", tx_on("null")),
        (
            "wallet_sendCalls",
            batch_params(&format!(r#"[{{"to":"{SPENDER}","value":"0x1"}}]"#), None),
        ),
    ] {
        let mut sut = boot();
        sut.dispatch(Arrive::extension("rid-89", method, &params, 137).event());
        assert_eq!(
            sut.view().request.expect("request").chain_id,
            137,
            "{method}"
        );
        sut.dispatch(approve(SignApproveOpts::default()));
        let ops = sut.resolve(Res::PreCheck { funding: None });
        assert!(
            matches!(ops.as_slice(), [Op::SignAndSubmit { chain_id: 137, .. }]),
            "{method} {params}: {ops:?}"
        );
    }
}

#[test]
fn in_app_request_naming_another_chain_still_switches_unchanged() {
    // No stamp: the wallet's chain IS the site's, so the request moves it —
    // the pre-089 rule, kept — and submits on the chain it named.
    for (method, params) in [
        ("eth_sendTransaction", tx_on(r#""0x89""#)),
        ("wallet_sendCalls", batch_on(r#""0x89""#)),
    ] {
        let mut sut = boot();
        let ops = sut.dispatch(Arrive::global("req-89", method, &params).event());
        assert!(
            !ops.iter().any(|op| matches!(op, Op::SendResponse { .. })),
            "{method}: not refused: {ops:?}"
        );
        let view = sut.view();
        assert_eq!(view.global_chain_id, 137, "{method}");
        assert_eq!(view.request.expect("request").chain_id, 137, "{method}");
        sut.dispatch(approve(SignApproveOpts::default()));
        let ops = sut.resolve(Res::PreCheck { funding: None });
        assert!(
            matches!(ops.as_slice(), [Op::SignAndSubmit { chain_id: 137, .. }]),
            "{method}: {ops:?}"
        );
    }
    // Typed data likewise switches to its domain's chain.
    let mut sut = boot();
    sut.dispatch(
        Arrive::global(
            "req-89t",
            "eth_signTypedData_v4",
            &typed_on(serde_json::json!(137)),
        )
        .event(),
    );
    assert_eq!(sut.view().global_chain_id, 137);
}

#[test]
fn approve_refuses_params_rewritten_to_name_another_chain() {
    // The chokepoint: whatever the sheet hands back is signed on the request's
    // chain only if it names no other one.
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-89", "eth_sendTransaction", &tx_on(r#""0x89""#), 137).event(),
    );
    let ops = sut.dispatch(approve(opts_with_override(&tx_on(r#""0x1""#))));
    assert!(
        matches!(ops.as_slice(), [op] if chain_mismatch_refusal(op)),
        "refused, nothing pre-checked or signed: {ops:?}"
    );
    assert!(
        sut.dispatch(approve(SignApproveOpts::default())).is_empty(),
        "answered once"
    );
}

#[test]
fn names_other_chain_reads_every_place_a_request_writes_its_chain() {
    let parse = |s: &str| serde_json::from_str::<serde_json::Value>(s).expect("json");
    let tx = parse(&tx_on(r#""0x64""#));
    assert!(request_names_other_chain("eth_sendTransaction", &tx, 1));
    assert!(!request_names_other_chain("eth_sendTransaction", &tx, 100));
    let batch = parse(&batch_on("100"));
    assert!(request_names_other_chain("wallet_sendCalls", &batch, 1));
    assert!(!request_names_other_chain("wallet_sendCalls", &batch, 100));
    let typed = parse(&typed_on(serde_json::json!("0x64")));
    assert!(request_names_other_chain("eth_signTypedData_v4", &typed, 1));
    assert!(!request_names_other_chain(
        "eth_signTypedData_v4",
        &typed,
        100
    ));
    // Names none: absent, null, a method that carries no chain.
    assert!(!request_names_other_chain(
        "eth_sendTransaction",
        &parse(&plain_send_params()),
        1
    ));
    assert!(!request_names_other_chain(
        "eth_sendTransaction",
        &parse(&tx_on("null")),
        1
    ));
    assert!(!request_names_other_chain(
        "personal_sign",
        &serde_json::json!([{"chainId": "0x64"}]),
        1
    ));
    // Present but unreadable is another chain.
    assert!(request_names_other_chain(
        "eth_sendTransaction",
        &parse(&tx_on(r#""0x""#)),
        1
    ));
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
    // The refusal is on the sheet, and its answer waits for the close (spec
    // 096 F8) — with no "try again": the same bytes are refused again.
    assert_eq!(
        response_count(&ops),
        0,
        "nothing is signed or answered yet: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::UnlimitedApproval)
    );
    assert!(!view.failure_retryable && !view.confirm_gate_open);
    let ops = sut.dispatch(Event::SwipeDismissed);
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
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.dispatch(Event::SwipeDismissed);
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
            signer: None,
        },
        now_ms: 11_000.0,
    });
    assert_eq!(
        response_count(&ops),
        0,
        "held while the failure shows: {ops:?}"
    );

    // A re-approve while it shows sends nothing; the reject is the close,
    // which sends the one error response — a failure, never a 4001.
    assert!(sut.dispatch(approve(SignApproveOpts::default())).is_empty());
    let ops = sut.dispatch(Event::RejectTapped);
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some("boom".to_owned())
        )),
        "reject after a failure = dismiss, which answers it"
    );
    assert_eq!(response_count(&ops), 1);
    assert_eq!(
        sut.view().surface,
        SignSurface::Hidden,
        "reject routed to dismiss"
    );
}

/// Every answer the core asked for in `ops`, as (transport, id, code) — `None`
/// for a success.
fn responses(ops: &[Op]) -> Vec<(String, String, Option<i32>)> {
    ops.iter()
        .filter_map(|op| match op {
            Op::SendResponse {
                transport_id,
                id,
                payload,
            } => Some((
                transport_id.clone(),
                id.clone(),
                match payload {
                    SignResponsePayload::Err { code, .. } => Some(*code),
                    SignResponsePayload::Ok { .. } => None,
                },
            )),
            _ => None,
        })
        .collect()
}

/// Approve a transaction as far as its signature: the pre-check answered,
/// `SignAndSubmit` out — the core is `Submitting`, so a close is a dismiss.
/// Returns every operation asked for on the way.
fn approve_to_submitting(sut: &mut Sut, id: &str) -> Vec<Op> {
    let mut ops =
        sut.dispatch(Arrive::global(id, "eth_sendTransaction", &plain_send_params()).event());
    ops.extend(sut.dispatch(approve(SignApproveOpts::default())));
    let submitting = sut.resolve(Res::PreCheck { funding: None });
    assert!(matches!(submitting.as_slice(), [Op::SignAndSubmit { .. }]));
    ops.extend(submitting);
    assert_eq!(sut.view().swipe_action, SignSwipeAction::Dismiss);
    ops
}

fn passkey_cancelled() -> Res {
    Res::Submit {
        outcome: SignSubmitOutcome::PasskeyCancelled,
        now_ms: NOW + 1_000.0,
    }
}

/// 083 (review): the prompt comes back unsigned, THEN the person closes —
/// the request is back on its form, so the close is a refusal. Over the whole
/// run, exactly one answer: that 4001.
#[test]
fn a_cancelled_prompt_then_a_close_answers_4001_once() {
    let mut sut = boot();
    let mut all = approve_to_submitting(&mut sut, "req-083a");

    let back = sut.resolve(passkey_cancelled());
    assert!(
        responses(&back).is_empty(),
        "the sheet is still up: {back:?}"
    );
    assert_eq!(sut.view().swipe_action, SignSwipeAction::Reject);
    all.extend(back);

    all.extend(sut.dispatch(Event::SwipeDismissed));
    all.extend(sut.dispatch(Event::SwipeDismissed));
    all.extend(sut.dispatch(Event::RejectTapped));
    assert_eq!(
        responses(&all),
        vec![(
            WP.to_owned(),
            "req-083a".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
    );
}

/// 083 (review): the person closes while the signature is still to come — a
/// dismiss, since the core is `Submitting` — THEN the prompt comes back
/// unsigned. This sent nothing, and the page's promise never settled (the
/// desktop's tab queued every later request behind it). Now, over the whole
/// run, exactly one answer: a 4001 to the request's own transport, settled as
/// the person's refusal.
#[test]
fn a_close_then_a_cancelled_prompt_answers_4001_once() {
    let mut sut = boot();
    let mut all = approve_to_submitting(&mut sut, "req-083b");

    let dismissed = sut.dispatch(Event::SwipeDismissed);
    assert!(
        responses(&dismissed).is_empty(),
        "a dismiss sends nothing yet: {dismissed:?}"
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    all.extend(dismissed);

    let back = sut.resolve(passkey_cancelled());
    let (code, kind, transport) = back
        .iter()
        .find_map(response_error)
        .expect("an error answer");
    assert_eq!(
        (code, kind, transport.as_str()),
        (CODE_USER_REJECTED, SignErrorKind::UserRejected, WP)
    );
    all.extend(back);
    all.extend(sut.dispatch(Event::SwipeDismissed));
    all.extend(sut.dispatch(Event::RejectTapped));
    assert_eq!(
        responses(&all),
        vec![(
            WP.to_owned(),
            "req-083b".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
    );

    // Settled as the person's refusal: the same rid never signs.
    sut.dispatch(Arrive::global("req-083b", "eth_sendTransaction", &plain_send_params()).event());
    assert_eq!(
        sut.view().notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Rejected
        })
    );
}

/// 083 (review): the same, with a newer request on the sheet by the time the
/// prompt comes back — the refusal goes to the OLD request (F2), and the new
/// one is untouched and can be approved.
#[test]
fn a_dismissed_request_cancelled_under_a_newer_one_answers_only_itself() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-083c", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    assert_eq!(sut.view().swipe_action, SignSwipeAction::Dismiss);
    sut.dispatch(Event::SwipeDismissed);
    sut.dispatch(Arrive::extension("rid-083d", "personal_sign", r#"["0xbeef","0x0"]"#, 1).event());

    let ops = sut.resolve(passkey_cancelled());
    assert_eq!(
        responses(&ops),
        vec![(
            WP.to_owned(),
            "req-083c".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
    );
    let view = sut.view();
    assert_eq!(view.request.expect("the newer request").id, "rid-083d");
    assert!(view.confirm_gate_open, "and it can still be approved");
}

/// 083 (review): a request replaced on the sheet while its signature was
/// still to come — no close at all — is off the sheet just the same: nobody
/// can retry or refuse it any more, so a prompt that comes back unsigned is
/// its refusal, once, and the newer request is untouched (the rule
/// `on_sponsorship` already keeps for a superseded pipeline).
#[test]
fn a_superseded_request_cancelled_answers_only_itself() {
    let mut sut = boot();
    let mut all = approve_to_submitting(&mut sut, "req-083i");
    all.extend(sut.dispatch(
        Arrive::extension("rid-083j", "personal_sign", r#"["0xbeef","0x0"]"#, 1).event(),
    ));
    assert_eq!(
        sut.view().request.expect("the newer request").id,
        "rid-083j"
    );

    all.extend(sut.resolve(passkey_cancelled()));
    assert_eq!(
        responses(&all),
        vec![(
            WP.to_owned(),
            "req-083i".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
    );
    let view = sut.view();
    assert_eq!(
        view.request.expect("still the newer request").id,
        "rid-083j"
    );
    assert!(view.confirm_gate_open, "and it can still be approved");
}

/// 083 (review): a request still on its sheet when the prompt comes back
/// unsigned is answered nothing — the pre-083 rule every client relies on
/// (the modal stays open for a retry; `dapp-connection.tsx:808-812`).
#[test]
fn a_cancelled_prompt_on_the_sheet_answers_nothing() {
    let mut sut = boot();
    let mut all = approve_to_submitting(&mut sut, "req-083k");
    all.extend(sut.resolve(passkey_cancelled()));
    assert!(responses(&all).is_empty(), "{all:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert!(view.confirm_gate_open, "open for a retry");
}

/// 083 (review): only a request whose page is still there is refused. A page
/// that went away already has its 4900 from the browser — with or without a
/// close before it, nothing more is sent — which is what every client did
/// before 083.
#[test]
fn a_page_already_gone_is_not_answered_again() {
    // Gone while signing, no close.
    let mut sut = boot();
    approve_to_submitting(&mut sut, "req-083e");
    sut.dispatch(Event::TransportDropped {
        transport_id: WP.to_owned(),
    });
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    assert!(responses(&sut.resolve(passkey_cancelled())).is_empty());

    // Closed, THEN gone, then the prompt comes back unsigned.
    let mut sut = boot();
    approve_to_submitting(&mut sut, "req-083f");
    sut.dispatch(Event::SwipeDismissed);
    sut.dispatch(Event::TransportDropped {
        transport_id: WP.to_owned(),
    });
    assert!(responses(&sut.resolve(passkey_cancelled())).is_empty());

    // Another transport dropping is not this page going away.
    let mut sut = boot();
    approve_to_submitting(&mut sut, "req-083g");
    sut.dispatch(Event::SwipeDismissed);
    sut.dispatch(Event::TransportDropped {
        transport_id: EXT.to_owned(),
    });
    assert_eq!(
        responses(&sut.resolve(passkey_cancelled())),
        vec![(
            WP.to_owned(),
            "req-083g".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
    );
}

/// 083 (review): a closed request the page sent again is back on the sheet
/// by the time its old prompt comes back unsigned — it is open for a retry
/// like any other (nothing sent), and the person's close of THAT sheet is
/// the one refusal.
#[test]
fn a_closed_request_back_on_the_sheet_stays_open() {
    let mut sut = boot();
    approve_to_submitting(&mut sut, "req-083h");
    sut.dispatch(Event::SwipeDismissed);
    sut.dispatch(Arrive::global("req-083h", "eth_sendTransaction", &plain_send_params()).event());
    assert_eq!(sut.view().request.expect("back").id, "req-083h");

    assert!(responses(&sut.resolve(passkey_cancelled())).is_empty());
    assert!(sut.view().confirm_gate_open, "open for a retry");
    assert_eq!(
        responses(&sut.dispatch(Event::SwipeDismissed)),
        vec![(
            WP.to_owned(),
            "req-083h".to_owned(),
            Some(CODE_USER_REJECTED)
        )]
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

/// 083 H2 review: the web answers `wallet_sendCalls` with its EIP-5792 batch
/// id — the userOpHash — the moment the relay accepts it, and says so
/// (`ReceiptPending`). The page gets that id, the durable record lands
/// PENDING under it first, and the tracker is handed the op to settle with
/// the real tx hash. Reported as `Succeeded`, the record closed "confirmed"
/// with the op hash as its tx hash — which Activity now shows.
#[test]
fn a_batch_answered_at_acceptance_stays_pending_for_the_tracker() {
    let mut sut = boot();
    let params = batch_params(
        &format!(r#"[{{"to":"{SPENDER}","data":"0x","value":"0x1"}}]"#),
        None,
    );
    sut.dispatch(Arrive::global("req-27b", "wallet_sendCalls", &params).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: "0xbatchid".to_owned(),
        },
        now_ms: 13_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.kind == SignRecordKind::DappTx
                && record.record_id == "dapp-13000-tx"
                && record.status == SignRecordStatus::Pending
                && record.user_op_hash == "0xbatchid"
                && record.result.is_empty()),
        "pending, under the op hash, with no tx hash: {ops:?}"
    );
    let handoff = sut.view().tracker_handoff.expect("the tracker gets the op");
    assert_eq!(handoff.user_op_hash, "0xbatchid");
    assert_eq!(handoff.record_ids, vec!["dapp-13000-tx".to_owned()]);
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xbatchid".to_owned()))),
        "the page still gets its batch id"
    );
    assert!(!ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })));
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
    // The page waits for the chain; at the window's end it hears "not
    // confirmed yet", never the local hash (083, owner ruling 2026-10-01).
    assert!(not_confirmed(&ops[0], LOCAL_OP), "{ops:?}");

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
        refusal: None,
        refusal_key: None,
        sender: None,
        user_op_hash: LOCAL_OP.to_owned(),
        chain_id: 100,
        record_ids: vec!["dapp-5000-tx".to_owned()],
        status,
        tx_hash: tx_hash.map(str::to_owned),
        polling: true,
        submitted_at_ms: Some(5_000.0),
        outcome,
        relay_tx_hash: None,
        relay_sent_at_ms: None,
    }
}

fn ok_answer(result: &str) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(result.into()),
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
                    fee_held: false,
                    relay_funding: false,
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
                fee_held: true,
                relay_funding: false,
            }
        );
        // The relay topping up its gas: its own line, not the fee hold's.
        assert_eq!(
            ending_state(
                ending,
                Some(&entry(
                    TrackStatus::RelayFunding,
                    TrackOutcome::Landing,
                    None
                ))
            ),
            SignEndingState::Following {
                user_op_hash: LOCAL_OP.to_owned(),
                outcome: TrackOutcome::Landing,
                fee_held: false,
                relay_funding: true,
            }
        );
        // No entry yet — or another op's — is the ring, never "confirmed".
        assert_eq!(
            ending_state(ending, None),
            SignEndingState::Following {
                user_op_hash: LOCAL_OP.to_owned(),
                outcome: TrackOutcome::Landing,
                fee_held: false,
                relay_funding: false,
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
            fee_held: false,
            relay_funding: false,
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
            fee_held: false,
            relay_funding: false,
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
        Event as TrackEvent, TrackFailure, TrackOperation as TOp, TrackRecordPatch,
        TrackRecordStatus, TrackSettlement, TrackShellResult as TRes, TxTracker,
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
        sender: None,
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
                settlement: Some(TrackSettlement {
                    moved: None,
                    failure: Some(TrackFailure::Reverted),
                }),
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
/// tracker over an on-chain record. The page now gets exactly one answer —
/// "not confirmed yet" (083; never the op hash, owner ruling 2026-10-01) —
/// the rid settles Submitted, the record is left to the tracker, and the
/// sheet is not an error.
#[test]
fn after_the_hand_off_a_failure_is_answered_not_confirmed() {
    let failures = [
        SignSubmitOutcome::Failed {
            message: "All bundler endpoints failed".to_owned(),
            refused: false,
            signer: None,
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
            ops.iter().filter_map(response_ok).next().is_none(),
            "{outcome:?}: never Ok with the op hash: {ops:?}"
        );
        assert_eq!(
            ops.iter().filter(|op| not_confirmed(op, LOCAL_OP)).count(),
            1,
            "{outcome:?}: one answer, not confirmed yet: {ops:?}"
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

fn response_count(ops: &[Op]) -> usize {
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
    assert_eq!(response_count(&ops), 0);
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
                signer: None,
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
    // Held while the sheet shows it (spec 096 F8); the close answers once.
    assert_eq!(response_count(&ops), 0, "{ops:?}");
    assert!(sut.view().failure_retryable, "nothing was sent: try again");
    let closed = sut.dispatch(Event::SwipeDismissed);
    assert_eq!(response_count(&closed), 1, "{closed:?}");
    let answer = closed.iter().find_map(err_detail);
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
    assert_eq!(view.surface, SignSurface::Hidden, "closed");
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
                signer: None,
            },
            now_ms: 9_000.0,
        },
    );
    assert_eq!(response_count(&ops), 0, "held for the sheet (096 F8)");
    let view = sut.view();
    assert!(view.failure_refused);
    assert!(!view.failure_retryable, "a refusal is not retried");
    let closed = sut.dispatch(Event::SwipeDismissed);
    assert_eq!(response_count(&closed), 1);
    assert_eq!(
        closed.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned())
        ))
    );
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
                signer: None,
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
/// The page gets ONE answer — -32603, "refused, nothing was sent" — when the
/// person closes the sheet that says so (spec 097 N4), and the window-end
/// result that follows is dropped: no later Ok.
#[test]
fn a_rejection_the_tracker_learns_is_answered_refused_once() {
    let mut sut = submitting("req-t1");
    written_ahead(&mut sut, "req-t1");
    sut.dispatch(op_submitted("req-t1", LOCAL_OP, false));
    assert!(sut
        .dispatch(op_tracked(TrackStatus::Pending, None, 12_000.0))
        .is_empty());
    let ops = sut.dispatch(op_tracked(TrackStatus::Rejected, None, 20_000.0));
    assert_eq!(response_count(&ops), 0, "held while it shows: {ops:?}");
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
    let closed = sut.dispatch(Event::SwipeDismissed);
    assert_eq!(response_count(&closed), 1, "{closed:?}");
    assert_eq!(
        closed.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned())
        ))
    );
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
/// -32603 with the not-sent sentence, inside the window, on the close of the
/// sheet that says so (spec 097 N4). No "Try again": the rid went to the
/// relay and is settled (⑧) — sending again is the page's to ask.
#[test]
fn a_not_sent_verdict_the_tracker_proves_is_answered_not_sent_once() {
    let mut sut = submitting("req-t2");
    written_ahead(&mut sut, "req-t2");
    sut.dispatch(op_submitted("req-t2", LOCAL_OP, true));
    let ops = sut.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0));
    assert_eq!(response_count(&ops), 0, "held while it shows: {ops:?}");
    let view = sut.view();
    assert!(!view.failure_refused);
    assert!(!view.failure_retryable, "the rid already went to the relay");
    assert!(
        sut.dispatch(Event::RetryTapped).is_empty(),
        "a retry the view does not offer does nothing"
    );
    assert!(sut
        .dispatch(op_tracked(TrackStatus::NotSent, None, 90_000.0))
        .is_empty());
    let closed = sut.dispatch(Event::DismissTapped);
    assert_eq!(response_count(&closed), 1, "{closed:?}");
    assert_eq!(
        closed.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(NOT_SENT_DAPP_DETAIL.to_owned())
        ))
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
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
    assert_eq!(response_count(&ops), 1);
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

    // A reverted op is answered the revert, naming its transaction — never
    // the hash a site would read as done (083, owner ruling 2026-10-01).
    let mut reverted = submitting("req-t4");
    reverted.dispatch(op_submitted("req-t4", LOCAL_OP, false));
    let ops = reverted.dispatch(op_tracked(TrackStatus::Dropped, Some(LANDED_TX), 30_000.0));
    assert!(
        ops.iter().filter_map(response_ok).next().is_none(),
        "{ops:?}"
    );
    assert_eq!(
        ops.iter().filter_map(err_detail).collect::<Vec<_>>(),
        vec![(
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(reverted_detail(LANDED_TX))
        )]
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the tracker closed it: {ops:?}"
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
    assert_eq!(response_count(&ops), 1);
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
            refused: false,
            signer: None,
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
    assert_eq!(response_count(&ops), 1, "{ops:?}");
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
    // A may-have-been-sent op is still answered "not sent" (EX-S5) — on the
    // close of the sheet that says so (spec 097 N4).
    let mut maybe = submitting("req-ns2");
    written_ahead(&mut maybe, "req-ns2");
    maybe.dispatch(op_submitted("req-ns2", LOCAL_OP, true));
    let ops = maybe.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0));
    assert_eq!(response_count(&ops), 0, "held while it shows: {ops:?}");
    assert!(maybe.view().error.is_some());
    assert_eq!(response_count(&maybe.dispatch(Event::SwipeDismissed)), 1);
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
                fee_held: false,
                relay_funding: false,
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
                signer: None,
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
            sender: Some(ACCT0.to_owned()),
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
    assert_eq!(response_count(&ops), 1, "{ops:?}");
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
    assert_eq!(response_count(&ops), 1, "{ops:?}");
    let handoff = handoff_of(&pending);
    assert_eq!(handoff.record_ids, vec![record.record_id]);
    assert!(handoff.maybe_sent && !handoff.admitted, "{handoff:?}");
}

// ===========================================================================
// 083 — one honest answer for a transaction
// ===========================================================================

const OP_HASH: &str = "0xabababababababababababababababababababababababababababababababab";
const TX_HASH: &str = "0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

/// A transaction approved and accepted by the relay (`OpSubmitted` at
/// `NOW`, its pending record written) — its page not yet answered.
fn submitted(id: &str) -> Sut {
    let mut sut = boot();
    sut.dispatch(Arrive::global(id, "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.dispatch(Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: OP_HASH.to_owned(),
        now_ms: NOW,
        maybe_sent: false,
        submit_block: None,
    });
    sut.resolve_matching(
        |op| matches!(op, Op::PersistRecord { .. }),
        Res::RecordPersisted,
    );
    sut
}

/// One answer the page got: `Ok(result)` or `Err((code, message))`.
type PageAnswer = Result<Option<String>, (i32, Option<String>)>;

/// Every answer the page got in `ops`: `Ok(result)` or `Err(code, message)`.
fn answers(ops: &[Op]) -> Vec<PageAnswer> {
    ops.iter()
        .filter_map(|op| match op {
            Op::SendResponse {
                payload: SignResponsePayload::Ok { result },
                ..
            } => Some(Ok(result.as_ref().map(page_text))),
            Op::SendResponse {
                payload: SignResponsePayload::Err { code, message, .. },
                ..
            } => Some(Err((*code, message.clone()))),
            _ => None,
        })
        .collect()
}

/// S2: the op was included and REVERTED. The bundle transaction's own status
/// is 0x1, so its hash read on the site's node says "done" — Uniswap showed a
/// swap that moved nothing, and Vela's record said confirmed. The page gets
/// ONE error instead, the record closes failed, the sheet shows the failure,
/// and nothing after that answers the page again.
#[test]
fn a_reverted_operation_is_answered_one_error_and_its_record_fails() {
    let mut sut = submitted("req-rv");
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::Reverted {
                user_op_hash: OP_HASH.to_owned(),
                tx_hash: TX_HASH.to_owned(),
            },
            now_ms: NOW + 12_000.0,
        },
    );
    assert_eq!(
        answers(&ops),
        vec![Err((
            CODE_INTERNAL,
            Some(format!("{REVERTED_MESSAGE} ({TX_HASH})"))
        ))],
        "one error, for the developer, naming the transaction: {ops:?}"
    );
    assert_eq!(
        REVERTED_MESSAGE,
        "The transaction was included but reverted"
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the record is the tracker's to close failed (spec 082 RA8): {ops:?}"
    );
    assert_eq!(
        sut.view()
            .tracker_handoff
            .map(|handoff| handoff.user_op_hash),
        Some(OP_HASH.to_owned()),
        "the tracker holds the op"
    );
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::UpdateRecord {
                close: SignRecordClose::Confirmed { .. },
                ..
            }
        )),
        "never 已确认: {ops:?}"
    );
    // The failure stays on screen as the sheet's ending (spec 082 RA8): the
    // answer reads as a revert naming its transaction, drawn at once.
    let ending = ending_in(&ops, "eth_sendTransaction", Some(OP_HASH)).expect("an ending");
    assert_eq!(
        ending,
        SignEnding::Reverted {
            tx_hash: TX_HASH.to_owned(),
            user_op_hash: Some(OP_HASH.to_owned()),
        }
    );
    assert_eq!(
        ending_state(&ending, None),
        SignEndingState::Reverted {
            tx_hash: TX_HASH.to_owned()
        }
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);

    // Once: no close of the sheet, nor the page asking again, answers more.
    sut.resolve(Res::Responded);
    assert!(answers(&sut.dispatch(Event::RejectTapped)).is_empty());
    assert!(answers(&sut.dispatch(Event::SwipeDismissed)).is_empty());
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    let ops =
        sut.dispatch(Arrive::global("req-rv", "eth_sendTransaction", &plain_send_params()).event());
    assert!(answers(&ops).is_empty(), "a replay is never re-signed");
    assert_eq!(
        sut.view().notice,
        Some(SignNotice::AlreadySettled {
            outcome: SignSettledOutcome::Submitted
        })
    );
}

/// A shell that saw no `OpSubmitted` still leaves the record before the
/// answer — and a reverted op's record ends failed, never confirmed.
#[test]
fn a_reverted_operation_without_op_submitted_records_first_then_one_error() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-rv2", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Reverted {
            user_op_hash: OP_HASH.to_owned(),
            tx_hash: TX_HASH.to_owned(),
        },
        now_ms: 9_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.status == SignRecordStatus::Pending && record.user_op_hash == OP_HASH),
        "the record first, and no answer yet: {ops:?}"
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the tracker, handed the op, closes it (spec 082 RA8): {ops:?}"
    );
    assert_eq!(
        answers(&ops),
        vec![Err((
            CODE_INTERNAL,
            Some(format!("{REVERTED_MESSAGE} ({TX_HASH})"))
        ))]
    );
    assert_eq!(
        sut.view().surface,
        SignSurface::Hidden,
        "the ending draws it"
    );
}

/// S3: while the operation has not landed the page is not answered at all —
/// not at 90 s, not when the person closes the column — and when the
/// transaction hash arrives, minutes later, THAT is the answer: never the
/// op hash, which no node the site asks will ever know.
#[test]
fn no_answer_until_the_transaction_hash_then_that_hash() {
    let mut sut = submitted("req-slow");
    assert!(
        matches!(sut.outstanding().as_slice(), [Op::SignAndSubmit { .. }]),
        "the pipeline is still out — the page is waiting: {:?}",
        sut.outstanding()
    );
    // Ninety seconds on: the person closes the column. The request goes on.
    let ops = sut.dispatch(Event::DismissTapped);
    assert!(answers(&ops).is_empty(), "a close answers nothing: {ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);

    // Five minutes in, the relay reports the bundle transaction.
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: TX_HASH.to_owned(),
        },
        now_ms: NOW + 300_000.0,
    });
    assert_eq!(answers(&ops), vec![Ok(Some(TX_HASH.to_owned()))], "{ops:?}");
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the tracker alone closes the record (spec 082 RA8): {ops:?}"
    );
}

/// S3: the cap. Ten minutes — the tracker's own "not landing soon" line —
/// with no transaction: the page hears THAT, as an error, never the op hash
/// as if it were a transaction. The record stays pending for the tracker (a
/// timeout is not a failure), and the sheet keeps following the operation,
/// which may still land; closing it answers nothing more.
#[test]
fn at_the_cap_the_page_hears_not_confirmed_never_the_op_hash() {
    use vela_core::app::tx_tracker::SLOW_POLL_AFTER_MS;
    assert_eq!(PAGE_WAIT_CAP_MS, SLOW_POLL_AFTER_MS);
    assert_eq!(PAGE_WAIT_CAP_MS, 600_000.0);

    let mut sut = submitted("req-cap");
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::SignAndSubmit { .. }),
        Res::Submit {
            outcome: SignSubmitOutcome::NotConfirmed {
                user_op_hash: OP_HASH.to_owned(),
            },
            now_ms: NOW + PAGE_WAIT_CAP_MS,
        },
    );
    assert_eq!(
        answers(&ops),
        vec![Err((
            CODE_INTERNAL,
            Some(format!(
                "{NOT_CONFIRMED_MESSAGE} (user operation {OP_HASH})"
            ))
        ))],
        "{ops:?}"
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::UpdateRecord { .. })),
        "the record is the tracker's: {ops:?}"
    );
    // Not a failure — it may still land: the sheet hands over to an ending
    // that follows the operation (spec 082 RA8).
    assert_eq!(
        ending_in(&ops, "eth_sendTransaction", Some(OP_HASH)),
        Some(SignEnding::StillConfirming {
            user_op_hash: OP_HASH.to_owned()
        })
    );
    let view = sut.view();
    assert_eq!(view.error, None, "not a failure — it may still land");
    assert_eq!(view.surface, SignSurface::Hidden);
    assert_eq!(
        view.tracker_handoff.map(|handoff| handoff.user_op_hash),
        Some(OP_HASH.to_owned())
    );
    sut.resolve(Res::Responded);
    assert!(answers(&sut.dispatch(Event::SwipeDismissed)).is_empty());
}

#[test]
fn not_confirmed_without_op_submitted_records_pending_then_answers() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-cap2", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::NotConfirmed {
            user_op_hash: OP_HASH.to_owned(),
        },
        now_ms: 9_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.status == SignRecordStatus::Pending && record.user_op_hash == OP_HASH),
        "{ops:?}"
    );
    assert_eq!(
        sut.view().tracker_handoff.map(|handoff| handoff.record_ids),
        Some(vec!["dapp-9000-tx".to_owned()])
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(ops.len(), 1, "the answer alone — no record patch: {ops:?}");
    assert!(matches!(
        answers(&ops).as_slice(),
        [Err((CODE_INTERNAL, _))]
    ));
    assert_eq!(
        sut.view().surface,
        SignSurface::Hidden,
        "the ending follows it"
    );
}

/// S3b: the relay refuses because ANOTHER operation of the account holds the
/// nonce — the approval, still pending, when the swap is submitted. Its hash
/// is never this request's: a shell used to answer the swap with it, and
/// Uniswap called the swap done when only the approval had happened. The
/// core's judgement tells the two apart; the request that could not go out
/// gets one clear error, and nothing is recorded under the other op.
#[test]
fn a_previous_pending_operation_is_never_this_requests_answer() {
    use vela_core::user_op::{existing_op, ExistingOp};
    let approval = "0x1111111111111111111111111111111111111111111111111111111111111111";
    let swap = "0x2222222222222222222222222222222222222222222222222222222222222222";
    let relay_said = format!("AA25 invalid account nonce [existingHash:{approval}]");
    assert_eq!(
        existing_op(&relay_said, swap),
        Some(ExistingOp::Another(approval.to_owned())),
        "another operation's hash"
    );
    assert_eq!(
        existing_op(&relay_said, approval),
        Some(ExistingOp::ThisOne(approval.to_owned())),
        "…unless it is this very operation, submitted twice"
    );

    // What the desktop reports for the swap instead (executor `user_op`):
    // nothing went out, so no `OpSubmitted`, and one error.
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-swap", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "Another transaction from this account was still pending, so this one was not sent. Try again.".to_owned(),
            refused: false,
            signer: None,
        },
        now_ms: NOW,
    });
    assert!(
        answers(&ops).is_empty(),
        "held for the sheet (096 F8): {ops:?}"
    );
    let closed = sut.dispatch(Event::SwipeDismissed);
    let answered = answers(&closed);
    assert!(
        matches!(answered.as_slice(), [Err((CODE_INTERNAL, Some(message)))]
            if !message.contains(approval)),
        "{closed:?}"
    );
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::PersistRecord { .. } | Op::UpdateRecord { .. })),
        "no record under the approval's hash: {ops:?}"
    );
    assert_eq!(sut.view().tracker_handoff, None);
}

// ===========================================================================
// 083 H2 — the record says what the transaction did
// ===========================================================================

/// Approve `params` with the sheet's `intent`, submit, and return the intent
/// the pending record carries.
fn recorded_intent(id: &str, method: &str, params: &str, intent: Option<&str>) -> Option<String> {
    let mut sut = boot();
    sut.dispatch(Arrive::global(id, method, params).event());
    sut.dispatch(approve(SignApproveOpts {
        intent: intent.map(str::to_owned),
        ..SignApproveOpts::default()
    }));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    let ops = sut.dispatch(Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 9_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    match ops.as_slice() {
        [Op::PersistRecord { record }] => record.intent.clone(),
        other => panic!("the pending record first: {other:?}"),
    }
}

/// A plain native send has no decoded result, so the sheet hands no intent;
/// the record says "Send" — the word its confirm already read — so Activity
/// can show "Send 1 ETH" rather than a contract interaction.
#[test]
fn a_plain_native_send_records_the_send_intent() {
    assert_eq!(
        recorded_intent("req-h2a", "eth_sendTransaction", &plain_send_params(), None),
        Some("Send".to_owned())
    );
}

/// Calldata nobody decoded stays without an intent (the shell reads
/// "Contract interaction"), and the sheet's own decoded intent always wins.
#[test]
fn a_contract_call_records_only_the_sheets_intent() {
    let call = tx_params("0xdeadbeef");
    assert_eq!(
        recorded_intent("req-h2b", "eth_sendTransaction", &call, None),
        None
    );
    assert_eq!(
        recorded_intent("req-h2c", "eth_sendTransaction", &call, Some("Swap")),
        Some("Swap".to_owned())
    );
    assert_eq!(
        recorded_intent(
            "req-h2d",
            "eth_sendTransaction",
            &plain_send_params(),
            Some("Transfer")
        ),
        Some("Transfer".to_owned())
    );
}

/// 083: the revert's error names its transaction in a form a shell can read
/// back — and nothing else reads as one.
#[test]
fn a_reverted_detail_names_its_transaction() {
    let detail = reverted_detail(TX_HASH);
    assert_eq!(detail, format!("{REVERTED_MESSAGE} ({TX_HASH})"));
    assert_eq!(reverted_transaction(&detail), Some(TX_HASH));
    assert_eq!(reverted_transaction(REVERTED_MESSAGE), None);
    assert_eq!(
        reverted_transaction(&format!("{REVERTED_MESSAGE} ()")),
        None
    );
    assert_eq!(
        reverted_transaction(&format!("{NOT_CONFIRMED_MESSAGE} (user operation 0xab)")),
        None
    );
}

// ===========================================================================
// 083 F1 — the record keeps what the sheet's simulation showed
// ===========================================================================

/// What the sheet drew under "Balance changes" for a 0.1 USDC -> ETH swap.
fn sheet_changes() -> Vec<TrustSimJudgment> {
    vec![
        TrustSimJudgment::Erc20Trusted {
            token: TOKEN.to_owned(),
            delta: "-100000".to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 6,
            in_trusted_set: true,
        },
        TrustSimJudgment::Native {
            delta: "37000000000000".to_owned(),
        },
    ]
}

/// Approve `params` carrying `changes` on the slide, submit, and return what
/// the pending record keeps.
fn recorded_changes(
    id: &str,
    method: &str,
    params: &str,
    changes: Option<Vec<TrustSimJudgment>>,
) -> Option<Vec<TrustSimJudgment>> {
    let mut sut = boot();
    sut.dispatch(Arrive::global(id, method, params).event());
    sut.dispatch(approve(SignApproveOpts {
        balance_changes: changes,
        ..SignApproveOpts::default()
    }));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    let ops = sut.dispatch(Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 9_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    match ops.as_slice() {
        [Op::PersistRecord { record }] => record.balance_changes.clone(),
        other => panic!("the pending record first: {other:?}"),
    }
}

/// The lines the person saw on the slide are the lines the record keeps —
/// so Activity can say "−0.1 USDC" for a swap it could only call a contract
/// interaction before.
#[test]
fn the_record_keeps_the_balance_changes_the_sheet_showed() {
    assert_eq!(
        recorded_changes(
            "req-f1a",
            "eth_sendTransaction",
            &tx_params("0xdeadbeef"),
            Some(sheet_changes())
        ),
        Some(sheet_changes())
    );
}

/// A page cannot write them. They ride the approve and nothing else: a
/// request that carries a `balance_changes` or an `assetChanges` of its own
/// records none, and with the sheet's, records exactly the sheet's.
#[test]
fn a_page_cannot_write_the_records_balance_changes() {
    let forged = format!(
        r#"[{{"to":"{TOKEN}","data":"0xdeadbeef","value":"0x0",
            "balance_changes":[{{"type":"native","delta":"999999999999999999999"}}],
            "assetChanges":{{"ok":true,"engine":"rpc","changes":[{{"kind":"native","delta":"999"}}]}}}}]"#
    );
    assert_eq!(
        recorded_changes("req-f1b", "eth_sendTransaction", &forged, None),
        None,
        "nothing the page sent is a balance change"
    );
    assert_eq!(
        recorded_changes(
            "req-f1c",
            "eth_sendTransaction",
            &forged,
            Some(sheet_changes())
        ),
        Some(sheet_changes()),
        "the sheet's, not the page's"
    );
}

/// An empty list is nothing to keep, and a signature moves nothing — its
/// record keeps no lines even if a shell handed some over.
#[test]
fn a_signature_or_an_empty_list_keeps_no_changes() {
    assert_eq!(
        recorded_changes(
            "req-f1d",
            "eth_sendTransaction",
            &tx_params("0xdeadbeef"),
            Some(Vec::new())
        ),
        None
    );

    let mut sut = boot();
    sut.dispatch(Arrive::global("req-f1e", "personal_sign", r#"["0xdead","0x0"]"#).event());
    let ops = sut.dispatch(approve(SignApproveOpts {
        balance_changes: Some(sheet_changes()),
        ..SignApproveOpts::default()
    }));
    assert!(matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]));
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: "0xsig".to_owned(),
        },
        now_ms: 9_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { record }]
            if record.kind == SignRecordKind::SignMessage && record.balance_changes.is_none()),
        "{ops:?}"
    );
}

/// The records written without an `OpSubmitted` keep them too: a batch
/// answered at acceptance (persisted pending under its op hash) and a batch
/// that answered with its result.
#[test]
fn every_transaction_record_keeps_the_changes() {
    let params = batch_params(
        &format!(r#"[{{"to":"{SPENDER}","data":"0xdeadbeef","value":"0x0"}}]"#),
        None,
    );
    for (id, outcome) in [
        (
            "req-f1f",
            SignSubmitOutcome::ReceiptPending {
                user_op_hash: "0xbatchid".to_owned(),
            },
        ),
        (
            "req-f1g",
            SignSubmitOutcome::Succeeded {
                result: "0xbatchid".to_owned(),
            },
        ),
    ] {
        let mut sut = boot();
        sut.dispatch(Arrive::global(id, "wallet_sendCalls", &params).event());
        sut.dispatch(approve(SignApproveOpts {
            balance_changes: Some(sheet_changes()),
            ..SignApproveOpts::default()
        }));
        sut.resolve(Res::PreCheck { funding: None });
        let ops = sut.resolve(Res::Submit {
            outcome,
            now_ms: 13_000.0,
        });
        assert!(
            matches!(ops.as_slice(), [Op::PersistRecord { record }]
                if record.balance_changes.as_deref() == Some(sheet_changes().as_slice())),
            "{id}: {ops:?}"
        );
    }
}

/// Optional on the wire both ways: a shell that predates the field approves
/// as before, and a record without lines carries no key for them.
#[test]
fn the_balance_changes_are_optional_on_the_wire() {
    let opts: SignApproveOpts = serde_json::from_str(
        r#"{"max_fee_per_gas":null,"bundler_cost_wei":null,"gas_fee_token":null,
            "quoted_fee":null,"fee_collector":null,"params_override_json":null,"intent":null}"#,
    )
    .expect("an older shell's approve");
    assert_eq!(opts.balance_changes, None);
    let out = serde_json::to_value(&opts).expect("serializes");
    assert!(out.get("balance_changes").is_none());

    let mut sut = boot();
    sut.dispatch(Arrive::global("req-f1h", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "req-f1h".to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 9_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    let [Op::PersistRecord { record }] = ops.as_slice() else {
        panic!("the pending record first: {ops:?}");
    };
    let out = serde_json::to_value(record).expect("serializes");
    assert!(out.get("balance_changes").is_none());
}

// ===========================================================================
// Spec 096 F1 — a call's value is read once, by the card's rule
// ===========================================================================

/// PancakeSwap's BNB → USDC on BNB Chain (2026-10-02): the request every shell
/// must send as 0.003 BNB — canonical already, so it passes byte for byte.
const PCS_ROUTER: &str = "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4";

fn pcs_swap(value: &str) -> String {
    format!(r#"[{{"from":"{ACCT0}","to":"{PCS_ROUTER}","value":{value},"data":"0x3593564c"}}]"#)
}

fn submitted_params(sut: &mut Sut) -> String {
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    match ops.as_slice() {
        [Op::SignAndSubmit { params_json, .. }] => params_json.clone(),
        other => unreachable!("{other:?}"),
    }
}

#[test]
fn a_native_value_reaches_the_submit_as_the_dapp_wrote_it() {
    let mut sut = boot();
    let params = pcs_swap(r#""0xaa87bee538000""#);
    sut.dispatch(Arrive::global("req-v1", "eth_sendTransaction", &params).event());
    assert_eq!(sut.view().request.expect("sheet").params_json, params);
    assert_eq!(submitted_params(&mut sut), params, "byte for byte");
}

#[test]
fn a_readable_value_is_carried_on_canonically() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::global(
            "req-v2",
            "eth_sendTransaction",
            &pcs_swap(r#""0x000AA87BEE538000""#),
        )
        .event(),
    );
    let shown: serde_json::Value =
        serde_json::from_str(&sut.view().request.expect("sheet").params_json).expect("json");
    assert_eq!(shown[0]["value"], "0xaa87bee538000");
    let sent: serde_json::Value = serde_json::from_str(&submitted_params(&mut sut)).expect("json");
    assert_eq!(
        sent[0]["value"], "0xaa87bee538000",
        "the card's text is the submit's"
    );
}

#[test]
fn an_unreadable_value_is_refused_before_any_sheet() {
    // The stripped text the web submit used to make; decimal; a JSON number.
    for value in [
        r#""aa87bee538000""#,
        r#""3000000000000000""#,
        "3000000000000000",
    ] {
        let mut sut = boot();
        let ops = sut.dispatch(
            Arrive::extension("rid-v3", "eth_sendTransaction", &pcs_swap(value), 1).event(),
        );
        assert!(
            matches!(ops.as_slice(), [Op::SendResponse {
                payload: SignResponsePayload::Err {
                    code: CODE_INVALID_PARAMS,
                    kind: SignErrorKind::InvalidParams,
                    message: Some(message),
                },
                ..
            }] if message.contains("0x-prefixed hex quantity")),
            "{value}: {ops:?}"
        );
        assert_eq!(sut.view().surface, SignSurface::Hidden, "{value}");
    }
    // A batch is refused for one bad leg, never sent without it.
    let mut sut = boot();
    let calls = format!(
        r#"[{{"to":"{SPENDER}","data":"0x095ea7b3","value":"0x0"}},{{"to":"{PCS_ROUTER}","value":"1000"}}]"#
    );
    let ops = sut.dispatch(
        Arrive::global("req-v4", "wallet_sendCalls", &batch_params(&calls, None)).event(),
    );
    let (code, kind, _) = response_error(&ops[0]).expect("refusal");
    assert_eq!(
        (code, kind),
        (CODE_INVALID_PARAMS, SignErrorKind::InvalidParams)
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn a_batch_native_leg_is_kept_and_zero_legs_made_canonical() {
    let mut sut = boot();
    let calls = format!(
        r#"[{{"to":"{SPENDER}","data":"0x095ea7b3","value":"0x"}},{{"to":"{PCS_ROUTER}","value":"0xaa87bee538000","data":"0x3593564c"}}]"#
    );
    sut.dispatch(Arrive::global("req-v5", "wallet_sendCalls", &batch_params(&calls, None)).event());
    let sent: serde_json::Value = serde_json::from_str(&submitted_params(&mut sut)).expect("json");
    assert_eq!(sent[0]["calls"][0]["value"], "0x0");
    assert_eq!(sent[0]["calls"][1]["value"], "0xaa87bee538000");
}

// ===========================================================================
// Spec 096 F8 — a failure before anything was sent stays on screen
// ===========================================================================

fn failed_before_sending(sut: &mut Sut, id: &str, message: &str) -> Vec<Op> {
    sut.dispatch(
        Arrive::extension(
            id,
            "eth_sendTransaction",
            &pcs_swap(r#""0xaa87bee538000""#),
            1,
        )
        .event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: message.to_owned(),
            refused: false,
            signer: None,
        },
        now_ms: NOW,
    })
}

#[test]
fn a_failure_is_shown_and_its_answer_waits_for_the_close() {
    let mut sut = boot();
    let ops = failed_before_sending(&mut sut, "rid-f1", "Could not estimate gas");
    assert_eq!(response_count(&ops), 0, "the window stays: {ops:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert_eq!(
        view.error.as_ref().map(|e| e.kind),
        Some(SignErrorKind::SubmitFailed)
    );
    assert!(view.failure_retryable && !view.failure_refused);
    assert!(!view.confirm_gate_open, "no slide under a failure");
    assert_eq!(view.swipe_action, SignSwipeAction::Dismiss);

    let ops = sut.dispatch(Event::SwipeDismissed);
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some("Could not estimate gas".to_owned())
        ))
    );
    assert_eq!(response_count(&ops), 1);
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    // A later close sends nothing more.
    assert!(sut
        .dispatch(Event::DismissTapped)
        .iter()
        .all(|op| !matches!(op, Op::SendResponse { .. })));
}

#[test]
fn try_again_goes_back_to_review_and_submits_afresh() {
    let mut sut = boot();
    failed_before_sending(&mut sut, "rid-f2", "Could not estimate gas");
    let ops = sut.dispatch(Event::RetryTapped);
    assert_eq!(response_count(&ops), 0, "still unanswered");
    let view = sut.view();
    assert!(view.error.is_none() && !view.failure_retryable);
    assert!(view.confirm_gate_open, "the slide is live again");
    assert_eq!(view.swipe_action, SignSwipeAction::Reject);
    assert_eq!(view.request.expect("same request").id, "rid-f2");

    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { id, .. }] if id == "rid-f2"),
        "{ops:?}"
    );
    let ops = sut.resolve(submit_ok("0xabc"));
    assert!(
        matches!(ops.as_slice(), [Op::PersistRecord { .. }]),
        "{ops:?}"
    );
    let ops = sut.resolve(Res::RecordPersisted);
    assert_eq!(
        answers(&ops),
        vec![Ok(Some("0xabc".to_owned()))],
        "one answer, the transaction: {ops:?}"
    );
}

#[test]
fn a_refusal_is_not_retried() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-f3", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "AA23".to_owned(),
            refused: true,
            signer: None,
        },
        now_ms: NOW,
    });
    assert!(!sut.view().failure_retryable);
    assert!(sut.dispatch(Event::RetryTapped).is_empty());
    assert!(sut.view().error.is_some(), "still the refusal");
}

#[test]
fn a_new_request_answers_the_failure_it_replaces() {
    let mut sut = boot();
    failed_before_sending(&mut sut, "rid-f4", "boom");
    let ops =
        sut.dispatch(Arrive::global("req-f5", "personal_sign", r#"["0xdead","0x0"]"#).event());
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some("boom".to_owned())
        ))
    );
    let view = sut.view();
    assert_eq!(view.request.expect("new").id, "req-f5");
    assert!(view.error.is_none(), "the old failure's words went with it");
}

#[test]
fn a_failure_nobody_watches_is_answered_at_once() {
    let mut sut = boot();
    sut.dispatch(
        Arrive::extension("rid-f6", "eth_sendTransaction", &plain_send_params(), 1).event(),
    );
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    // Closed while it ran (the passkey is the shell's; the sheet went).
    sut.dispatch(Event::DismissTapped);
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "boom".to_owned(),
            refused: false,
            signer: None,
        },
        now_ms: NOW,
    });
    assert_eq!(response_count(&ops), 1, "{ops:?}");
}

// ===========================================================================
// Spec 097 N4 — a refusal after "Submitted" is shown, not swallowed
// ===========================================================================

/// The golden Safe of the pass, the account the request came from.
const GOLDEN_SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

/// The pass's request (`fixtures/dapp097/aave-withdraw-refused.json`): its
/// method, its params as the page sent them, its origin and its chain.
fn n4_request() -> (String, String, String, u32) {
    let path = format!(
        "{}/tests/fixtures/dapp097/aave-withdraw-refused.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let fixture: serde_json::Value = serde_json::from_str(&raw).expect("fixture json");
    let chain = fixture["chainId"].as_u64().expect("chainId");
    (
        fixture["method"].as_str().expect("method").to_owned(),
        fixture["params"].to_string(),
        fixture["origin"].as_str().expect("origin").to_owned(),
        u32::try_from(chain).expect("a chain id"),
    )
}

/// The pass's moment, on the extension's own path: the request stamped onto
/// its window's transport and its site's chain, slid, signed, written ahead,
/// and taken by the relay — the sheet says "Submitted" with the op's hash.
fn n4_submitted(rid: &str) -> Sut {
    let (method, params, origin, chain) = n4_request();
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, chain],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: GOLDEN_SAFE.to_owned(),
            credential_id: "cred-golden".to_owned(),
        }],
        active_index: 0,
    });
    sut.dispatch(Event::RequestArrived {
        id: rid.to_owned(),
        method,
        params_json: params,
        origin,
        transport_id: EXT.to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(chain),
        dapp: None,
        granted_address: Some(GOLDEN_SAFE.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
        first_party: false,
    });
    assert_eq!(sut.view().surface, SignSurface::Sheet, "the sheet is up");
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [Op::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    written_ahead(&mut sut, rid);
    sut.dispatch(op_submitted(rid, LOCAL_OP, false));
    let view = sut.view();
    assert_eq!(view.pending_op_hash.as_deref(), Some(LOCAL_OP), "Submitted");
    assert!(view.error.is_none());
    sut
}

fn is_refused_answer(op: &Op) -> bool {
    matches!(op, Op::SendResponse {
        transport_id,
        payload: SignResponsePayload::Err {
            code: CODE_INTERNAL,
            kind: SignErrorKind::SubmitFailed,
            message: Some(message),
        },
        ..
    } if transport_id == EXT && message == REFUSED_DAPP_DETAIL)
}

/// N4 (S3): the relay took the op, the sheet said "Submitted", and ~12 s
/// later the relay refused it. The answer went out with the verdict, the
/// extension worker closed the window on it, and the person never read why.
///
/// Now the refusal stays on the sheet in its own words, with Done and no
/// "Try again" (⑧, RJ3), and nothing — a re-approve, a retry, a later
/// verdict, the shell's late window end — answers the page. The close does,
/// exactly once, with the refused sentence. Fails on the old code: the
/// verdict itself answered.
#[test]
fn n4_a_refusal_after_submitted_stays_on_the_sheet_and_the_close_answers_it_once() {
    let mut sut = n4_submitted("rid-n4");

    let ops = sut.dispatch(op_tracked(TrackStatus::Rejected, None, 12_000.0));
    assert_eq!(response_count(&ops), 0, "held while it shows: {ops:?}");
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet, "the sheet stays");
    assert_eq!(view.request.as_ref().map(|r| r.id.as_str()), Some("rid-n4"));
    assert_eq!(
        view.error,
        Some(vela_core::app::sign_request::SignErrorNotice::new(
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned()),
        )),
        "the refusal, in its own words"
    );
    assert!(view.failure_refused);
    assert!(
        !view.failure_retryable,
        "refused, and the rid went out: Done only"
    );
    assert!(view.pending_op_hash.is_none(), "no longer 'Submitted'");
    assert!(!view.confirm_gate_open);
    assert_eq!(
        view.swipe_action,
        SignSwipeAction::Dismiss,
        "the close is Done"
    );
    assert_eq!(view.phase, SignPhase::Idle);

    // Nothing but the close answers it.
    assert!(sut.dispatch(Event::RetryTapped).is_empty());
    assert!(sut.dispatch(approve(SignApproveOpts::default())).is_empty());
    assert!(sut
        .dispatch(op_tracked(TrackStatus::Rejected, None, 13_000.0))
        .is_empty());
    let late = sut.resolve_matching(is_submit, {
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: "UserOperation reverted".to_owned(),
                refused: true,
                signer: None,
            },
            now_ms: 120_000.0,
        }
    });
    assert!(late.is_empty(), "the window's late end: {late:?}");
    assert!(sut.view().failure_refused, "still on screen");

    let closed = sut.dispatch(Event::SwipeDismissed);
    assert_eq!(response_count(&closed), 1, "{closed:?}");
    assert!(closed.iter().any(is_refused_answer), "{closed:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    assert!(sut.dispatch(Event::SwipeDismissed).is_empty(), "once");
    assert!(sut.dispatch(Event::DismissTapped).is_empty(), "once");
}

/// The same refusal with nobody looking — the sheet closed while the op was
/// on its way: the page is answered at once (the rule of 096 F8).
#[test]
fn n4_a_refusal_nobody_is_looking_at_is_answered_at_once() {
    let mut sut = n4_submitted("rid-n4b");
    assert!(
        sut.dispatch(Event::SwipeDismissed).is_empty(),
        "a close while it is on its way refuses nothing"
    );
    let ops = sut.dispatch(op_tracked(TrackStatus::Rejected, None, 12_000.0));
    assert_eq!(response_count(&ops), 1, "{ops:?}");
    assert!(ops.iter().any(is_refused_answer), "{ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    assert!(sut.dispatch(Event::SwipeDismissed).is_empty());
}

/// A new request taking the sheet answers the refusal it replaces, once —
/// its page must not wait on words nobody will see again.
#[test]
fn n4_a_new_request_answers_the_refusal_it_replaces() {
    let mut sut = n4_submitted("rid-n4c");
    sut.dispatch(op_tracked(TrackStatus::Rejected, None, 12_000.0));
    let ops = sut
        .dispatch(Arrive::extension("rid-n4d", "personal_sign", r#"["0xdead","0x0"]"#, 1).event());
    assert_eq!(response_count(&ops), 1, "{ops:?}");
    assert!(ops.iter().any(is_refused_answer), "{ops:?}");
    let view = sut.view();
    assert_eq!(view.request.expect("the new one").id, "rid-n4d");
    assert!(view.error.is_none() && !view.failure_refused);
}

/// The page went while the refusal showed: the browser already answered it
/// (4900); the sheet goes and nothing more is said — never a second answer.
#[test]
fn n4_a_page_gone_under_the_refusal_is_not_answered_again() {
    let mut sut = n4_submitted("rid-n4e");
    sut.dispatch(op_tracked(TrackStatus::Rejected, None, 12_000.0));
    let ops = sut.dispatch(Event::TransportDropped {
        transport_id: EXT.to_owned(),
    });
    assert_eq!(response_count(&ops), 0, "{ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    assert!(sut.dispatch(Event::SwipeDismissed).is_empty());
}

/// A "nothing was sent" verdict after the submit is held the same way — and
/// is not tried again from the sheet: the rid went to the relay (⑧).
#[test]
fn n4_not_sent_after_the_submit_is_held_with_no_retry() {
    let mut sut = n4_submitted("rid-n4f");
    let ops = sut.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0));
    // An op the relay accepted is never "not sent" (082 round-2 review)…
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(sut.view().pending_op_hash.as_deref(), Some(LOCAL_OP));
    // …so the case is a lost reply's.
    let mut sut = submitting("req-n4g");
    written_ahead(&mut sut, "req-n4g");
    sut.dispatch(op_submitted("req-n4g", LOCAL_OP, true));
    assert_eq!(
        response_count(&sut.dispatch(op_tracked(TrackStatus::NotSent, None, 87_000.0))),
        0
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert!(!view.failure_refused && !view.failure_retryable);
    assert!(sut.dispatch(Event::RetryTapped).is_empty());
    assert_eq!(response_count(&sut.dispatch(Event::SwipeDismissed)), 1);
}

/// A failure held before the submit still offers "Try again" (096 F8): the
/// rule that keeps it from a post-submit verdict is the rid's settlement,
/// nothing else.
#[test]
fn n4_a_failure_before_the_submit_still_offers_try_again() {
    let mut sut = submitting("req-n4h");
    written_ahead(&mut sut, "req-n4h");
    sut.resolve_matching(
        is_submit,
        Res::Submit {
            outcome: SignSubmitOutcome::Failed {
                message: NOT_SENT_DAPP_DETAIL.to_owned(),
                refused: false,
                signer: None,
            },
            now_ms: 9_000.0,
        },
    );
    assert!(sut.view().failure_retryable);
    sut.dispatch(Event::RetryTapped);
    assert!(sut.view().confirm_gate_open, "back to review");
}

/// A global chain switch while a failure is held answers that failure, in
/// its own words — never a 4001 "cancelled" for a request the person
/// approved and the network refused.
#[test]
fn n4_a_chain_switch_under_a_held_refusal_answers_the_refusal() {
    let mut sut = submitting("req-n4i");
    written_ahead(&mut sut, "req-n4i");
    sut.dispatch(op_submitted("req-n4i", LOCAL_OP, false));
    sut.dispatch(op_tracked(TrackStatus::Rejected, None, 12_000.0));
    let ops = sut.dispatch(Event::ChainSwitchRequested {
        id: Some("sw-n4".to_owned()),
        transport_id: Some(WP.to_owned()),
        chain_id_param: Some("0x89".to_owned()),
    });
    let answers = responses(&ops);
    assert_eq!(
        answers,
        vec![
            (WP.to_owned(), "req-n4i".to_owned(), Some(CODE_INTERNAL)),
            (WP.to_owned(), "sw-n4".to_owned(), None),
        ],
        "{ops:?}"
    );
    assert_eq!(
        ops.iter().find_map(err_detail),
        Some((
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(REFUSED_DAPP_DETAIL.to_owned())
        ))
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
    assert!(sut.dispatch(Event::SwipeDismissed).is_empty());
}

// ---------------------------------------------------------------------------
// Spec 099 R8 — the signer says how it failed
// ---------------------------------------------------------------------------

fn passkey_failed(signer: vela_core::app::FailureKind) -> Sut {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-99", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "The operation couldn't be completed".to_owned(),
            refused: false,
            signer: Some(signer),
        },
        now_ms: 11_000.0,
    });
    assert_eq!(response_count(&ops), 0, "held while the failure shows");
    sut
}

/// No passkey can be used here (an unsigned build): the sheet names the
/// signer, Try again is not offered (it cannot work), and the page's answer
/// carries the signer's kind.
#[test]
fn an_unavailable_passkey_is_named_and_not_retried() {
    let mut sut = passkey_failed(vela_core::app::FailureKind::NotSupported);
    let view = sut.view();
    assert_eq!(
        view.error.as_ref().map(|e| e.kind),
        Some(SignErrorKind::SignerUnavailable)
    );
    assert!(!view.failure_retryable);
    assert!(!view.confirm_gate_open);
    let ops = sut.dispatch(Event::RejectTapped);
    assert_eq!(
        ops.iter()
            .find_map(err_detail)
            .map(|(code, kind, _)| (code, kind)),
        Some((CODE_INTERNAL, SignErrorKind::SignerUnavailable))
    );
}

/// Spec 102: the account cannot sign here (its keys answer only on its page,
/// and this is the web; or no page for its domain is known here). The sheet
/// says why with the block's own words, Try again is not offered, and the
/// page hears -32603 in a calm sentence.
#[test]
fn a_blocked_venue_is_said_and_not_retried() {
    use vela_core::signing_venue::VenueBlock;
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-102", "personal_sign", r#"["0xdead","0x0"]"#).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::VenueBlocked {
            block: VenueBlock::NotOnWeb,
        },
        now_ms: 11_000.0,
    });
    assert_eq!(response_count(&ops), 0, "held while the failure shows");
    let view = sut.view();
    let error = view.error.clone().expect("a failure on the sheet");
    assert_eq!(error.kind, SignErrorKind::VenueBlocked);
    assert_eq!(error.venue_block, Some(VenueBlock::NotOnWeb));
    assert_eq!(
        error.venue_block.as_ref().map(VenueBlock::key),
        Some("settings.venue.blockedWeb")
    );
    assert!(!view.failure_retryable);
    let ops = sut.dispatch(Event::RejectTapped);
    assert_eq!(
        ops.iter()
            .find_map(err_detail)
            .map(|(code, kind, _)| (code, kind)),
        Some((CODE_INTERNAL, SignErrorKind::VenueBlocked))
    );
}

/// A prompt that failed for another reason may work next time: Try again.
#[test]
fn a_failed_passkey_prompt_can_be_tried_again() {
    let sut = passkey_failed(vela_core::app::FailureKind::Other);
    let view = sut.view();
    assert_eq!(
        view.error.as_ref().map(|e| e.kind),
        Some(SignErrorKind::SignerFailed)
    );
    assert!(view.failure_retryable);
}

/// A cancelled prompt is a cancel, whichever way the shell says it.
#[test]
fn a_cancel_reported_as_a_failure_is_a_cancel() {
    let sut = passkey_failed(vela_core::app::FailureKind::Cancelled);
    assert!(sut.view().error.is_none(), "a cancel is never an error");
}

// ===========================================================================
// One transaction in flight per account and network: wait for the first
// ===========================================================================

fn in_flight(sender: &str, chain_id: u32) -> Event {
    Event::InFlightOps {
        ops: vec![vela_core::app::tx_tracker::InFlightOp {
            sender: sender.to_lowercase(),
            chain_id,
            user_op_hash: "0xfirst".to_owned(),
        }],
    }
}

/// A transaction of an account whose previous one on the request's chain is
/// still going through waits for it: the gate is shut with its own block
/// (and line), a stale tap signs nothing, and it opens once the first is
/// final.
#[test]
fn a_transaction_waits_for_the_account_s_previous_one_on_its_chain() {
    use vela_core::app::sign_confirm::ConfirmBlock;
    let mut sut = boot();
    sut.dispatch(in_flight(ACCT0, 1));
    sut.dispatch(Arrive::global("req-wait", "eth_sendTransaction", &plain_send_params()).event());
    let view = sut.view();
    assert!(!view.confirm_gate_open);
    assert_eq!(view.confirm_block, Some(ConfirmBlock::PreviousPending));
    assert_eq!(
        ConfirmBlock::PreviousPending.key(false),
        Some("componentsUi.signing.confirmBlock.previousPending")
    );
    assert!(
        sut.dispatch(approve(SignApproveOpts::default())).is_empty(),
        "a tap from a stale frame starts nothing"
    );

    sut.dispatch(Event::InFlightOps { ops: vec![] });
    let view = sut.view();
    assert!(
        view.confirm_gate_open,
        "opens by itself once the first is final"
    );
    assert_eq!(
        sut.dispatch(approve(SignApproveOpts::default())).len(),
        1,
        "and signs"
    );
}

/// Another chain, another account, or a signature (which takes no nonce)
/// waits for nothing.
#[test]
fn only_a_transaction_of_the_same_account_on_the_same_chain_waits() {
    let mut sut = boot();
    sut.dispatch(in_flight(ACCT0, 137));
    sut.dispatch(Arrive::global("req-a", "eth_sendTransaction", &plain_send_params()).event());
    assert!(sut.view().confirm_gate_open, "another chain");

    let mut sut = boot();
    sut.dispatch(in_flight(ACCT1, 1));
    sut.dispatch(Arrive::global("req-b", "eth_sendTransaction", &plain_send_params()).event());
    assert!(sut.view().confirm_gate_open, "another account");

    let mut sut = boot();
    sut.dispatch(in_flight(ACCT0, 1));
    sut.dispatch(Arrive::global("req-c", "personal_sign", r#"["0x68656c6c6f","0x0"]"#).event());
    assert!(sut.view().confirm_gate_open, "a signature takes no nonce");
}
