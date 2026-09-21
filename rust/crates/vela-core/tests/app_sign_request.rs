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
    extract_request_chain_id, is_signing_method, method_kind, required_capabilities,
    sign_account_index, Event, SignAccountRef, SignApproveOpts, SignDappIdentity, SignErrorKind,
    SignFundingNeeded, SignFundingPresentation, SignMethodKind, SignNotice, SignOperation as Op,
    SignQuotedFee, SignRecordClose, SignRecordKind, SignRecordStatus, SignRequest,
    SignResponsePayload, SignSettledOutcome, SignShellResult as Res, SignSponsorship,
    SignSubmitOutcome, SignSurface, SignSwipeAction, CODE_INTERNAL, CODE_INVALID_PARAMS,
    CODE_UNAUTHORIZED, CODE_UNSUPPORTED_CAPABILITY, CODE_UNSUPPORTED_CHAIN, CODE_USER_REJECTED,
};

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

#[test]
fn tx_pending_record_persists_at_submission_then_flips_confirmed_in_place() {
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

    // Final result: respond, then flip the SAME record confirmed.
    let ops = sut.resolve(submit_ok("0xtxhash"));
    assert_eq!(
        response_ok(&ops[0]),
        Some((WP.to_owned(), Some("0xtxhash".to_owned())))
    );
    assert!(
        matches!(&ops[1], Op::UpdateRecord { record_id, close: SignRecordClose::Confirmed { tx_hash } }
            if record_id == "dapp-5000-tx" && tx_hash == "0xtxhash"),
        "same id, in place, never a second record: {ops:?}"
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn failed_submit_patches_the_pending_record_failed() {
    let mut sut = boot();
    sut.dispatch(Arrive::global("req-8", "eth_sendTransaction", &plain_send_params()).event());
    sut.dispatch(approve(SignApproveOpts::default()));
    sut.resolve(Res::PreCheck { funding: None });
    sut.dispatch(Event::OpSubmitted {
        id: "req-8".to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 6_000.0,
    });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Failed {
            message: "dropped from the network".to_owned(),
        },
        now_ms: 7_000.0,
    });
    assert!(
        matches!(&ops[0], Op::UpdateRecord { record_id, close: SignRecordClose::Failed }
            if record_id == "dapp-6000-tx"),
        "no eternal pending: {ops:?}"
    );
    let (code, kind, _) = response_error(&ops[1]).expect("error response");
    assert_eq!((code, kind), (CODE_INTERNAL, SignErrorKind::SubmitFailed));
    // Modal stays open with the error; closing is a dismiss.
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert_eq!(view.error.expect("error").kind, SignErrorKind::SubmitFailed);
    assert_eq!(view.swipe_action, SignSwipeAction::Dismiss);
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
// approval_guard composition — the never-unlimited submit chokepoint
// ===========================================================================

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
