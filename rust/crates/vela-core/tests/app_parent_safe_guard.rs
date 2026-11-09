//! §31 — the parent-Safe guard: a site may never make this account act for a
//! Safe it owns (a multisig it is a member of). Reported 2026-09-29
//! (disclosure appendix APPENDIX-PARENT).
//!
//! The vector `vectors/rules/parent-safe-guard.json` is the contract every
//! platform replays; this is its first reader. The machine half proves the
//! request is refused before it can be signed — at arrival from the list, after
//! an `isOwner` read for any other Safe, and again at the submit chokepoint —
//! that a read nobody could make refuses, and that the refusal is answered.
#![cfg(feature = "crux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::parent_safe_guard::{
    check, check_json, CheckInput, ParentReason, ParentVerdict, SafeOwnerAnswer, IS_OWNER,
};
use vela_core::app::sign_confirm::ConfirmBlock;
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignDappIdentity, SignErrorKind, SignOperation as Op,
    SignRequest, SignResponsePayload, SignShellResult, SignSurface, CODE_INTERNAL,
    CODE_INVALID_PARAMS,
};

const VECTOR: &str = include_str!("vectors/rules/parent-safe-guard.json");

fn vector() -> Value {
    serde_json::from_str(VECTOR).expect("the vector is JSON")
}

// ---------------------------------------------------------------------------
// The vector
// ---------------------------------------------------------------------------

#[test]
fn every_vector_case_is_the_cores_verdict() {
    let v = vector();
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 30, "the vector covers the shapes");
    for case in cases {
        let input: CheckInput = serde_json::from_value(case["input"].clone()).unwrap();
        let got = serde_json::to_value(check(&input)).unwrap();
        assert_eq!(got, case["expect"], "{}", case["name"]);
        // The JSON door every platform uses says the same.
        let through_json: Value =
            serde_json::from_str(&check_json(&case["input"].to_string()).unwrap()).unwrap();
        assert_eq!(through_json, case["expect"], "json door: {}", case["name"]);
    }
    assert_eq!(v["isOwnerSelector"], IS_OWNER);
}

#[test]
fn the_vector_names_every_shape_the_disclosure_did() {
    let v = vector();
    let names: Vec<&str> = v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for wanted in [
        "a: execTransaction",
        "b: approveHash",
        "c: SafeMessage",
        "c: SafeTx",
        "MultiSend",
        "unknown Safe, isOwner true",
        "isOwner unreadable",
        "PoC (APPENDIX-PARENT)",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(wanted)),
            "missing {wanted}"
        );
    }
}

#[test]
fn a_malformed_input_is_an_error_not_a_verdict() {
    assert!(check_json("{").is_err());
    assert!(check_json("{\"method\":1}").is_err());
}

// ---------------------------------------------------------------------------
// The machine
// ---------------------------------------------------------------------------

type Sut = DomainDriver<SignRequest>;

const WP: &str = "wp-1";
const ORIGIN: &str = "https://evil.example";
const NOW: f64 = 1_700_000_000_000.0;

fn signer() -> String {
    vector()["signer"].as_str().unwrap().to_owned()
}
fn parent() -> String {
    vector()["parent"].as_str().unwrap().to_owned()
}
fn unknown() -> String {
    vector()["unknown"].as_str().unwrap().to_owned()
}
fn chain() -> u32 {
    u32::try_from(vector()["chainId"].as_u64().unwrap()).unwrap()
}

/// The params of the vector case named `name`.
fn params_of(name: &str) -> (String, Value) {
    let v = vector();
    let case = v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no case {name}"))
        .clone();
    (
        case["input"]["method"].as_str().unwrap().to_owned(),
        case["input"]["params"].clone(),
    )
}

fn boot(known: &[String]) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 137, chain()],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: signer(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sut.dispatch(Event::MultisigsChanged {
        addresses: known.to_vec(),
    });
    sut
}

fn arrive_as(sut: &mut Sut, method: &str, params: &Value, first_party: bool) -> Vec<Op> {
    sut.dispatch(Event::RequestArrived {
        id: "rid-1".to_owned(),
        method: method.to_owned(),
        params_json: params.to_string(),
        origin: ORIGIN.to_owned(),
        transport_id: WP.to_owned(),
        dedicated_transport: false,
        per_request_chain: Some(chain()),
        dapp: Some(SignDappIdentity {
            name: "evil.example".to_owned(),
            url: Some(ORIGIN.to_owned()),
        }),
        granted_address: Some(signer()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
        first_party,
    })
}

fn arrive(sut: &mut Sut, method: &str, params: &Value) -> Vec<Op> {
    arrive_as(sut, method, params, false)
}

fn reads(ops: &[Op]) -> Vec<(u32, String, String, String)> {
    ops.iter()
        .filter_map(|op| match op {
            Op::ReadSafeOwner {
                chain_id,
                safe,
                account,
                data,
                ..
            } => Some((*chain_id, safe.clone(), account.clone(), data.clone())),
            _ => None,
        })
        .collect()
}

fn answered_parent_refusal(ops: &[Op]) -> bool {
    ops.iter().any(|op| {
        matches!(
            op,
            Op::SendResponse { payload: SignResponsePayload::Err { code, kind, .. }, .. }
                if *code == CODE_INTERNAL && *kind == SignErrorKind::ParentSafeBlocked
        )
    })
}

fn word(b: u8) -> String {
    format!("0x{:0>64}", format!("{b:x}"))
}

fn answer_read(sut: &mut Sut, safe: &str, answer: SafeOwnerAnswer) -> Vec<Op> {
    let safe = safe.to_owned();
    sut.resolve_matching(
        |op| matches!(op, Op::ReadSafeOwner { safe: s, .. } if *s == safe),
        // R22: an honest chain, read of two independent nodes alike.
        SignShellResult::SafeOwnerRead {
            id: "rid-1".to_owned(),
            chain_id: chain(),
            safe: safe.clone(),
            witness: Some(answer.clone()),
            answer,
            own_node: None,
        },
    )
}

/// Opens refused, unsignable, and is answered "the wallet refused" — never
/// 4001 — when the person closes it.
fn assert_refused_as(sut: &mut Sut, function: &str, reason: ParentReason, safe: &str) {
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet, "the sheet explains it");
    assert!(!view.confirm_gate_open, "there is nothing to confirm");
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Refused));
    let blocked = view.blocked.expect("the sheet carries the reason");
    assert_eq!(blocked.function, function);
    assert_eq!(blocked.parent.as_deref(), Some(safe));
    assert_eq!(blocked.reason, Some(reason));
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::ParentSafeBlocked)
    );
    // However the shell asks, nothing is signed.
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::SignAndSubmit { .. } | Op::CheckBundlerFunding { .. }
        )),
        "nothing is ever signed for a refused request: {ops:?}"
    );
    let ops = sut.dispatch(Event::SwipeDismissed);
    assert!(answered_parent_refusal(&ops), "answered: {ops:?}");
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

#[test]
fn a_takeover_exec_transaction_on_a_multisig_on_the_list_opens_refused() {
    let mut sut = boot(&[parent()]);
    let (method, params) =
        params_of("a: execTransaction(swapOwner) on a multisig on the list, v=1");
    let ops = arrive(&mut sut, &method, &params);
    assert!(reads(&ops).is_empty(), "the list decides: no read");
    assert!(
        !ops.iter().any(|op| matches!(op, Op::SendResponse { .. })),
        "the answer waits for the person to close the sheet"
    );
    assert_refused_as(&mut sut, "execTransaction", ParentReason::Member, &parent());
}

#[test]
fn approve_hash_on_a_multisig_on_the_list_opens_refused() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("b: approveHash on a multisig on the list");
    arrive(&mut sut, &method, &params);
    assert_refused_as(&mut sut, "approveHash", ParentReason::Member, &parent());
}

#[test]
fn a_safe_message_under_a_multisigs_domain_is_refused_not_signed_anyway() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("c: SafeMessage under a multisig's domain");
    arrive(&mut sut, &method, &params);
    assert_refused_as(&mut sut, "SafeMessage", ParentReason::Member, &parent());
}

#[test]
fn a_safe_tx_for_a_multisig_is_refused_in_the_parents_words() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("c: SafeTx under a multisig's domain");
    arrive(&mut sut, &method, &params);
    assert_refused_as(&mut sut, "SafeTx", ParentReason::Member, &parent());
}

#[test]
fn a_multisend_hiding_an_approval_is_refused() {
    let mut sut = boot(&[parent()]);
    let (method, params) =
        params_of("MultiSend: the takeover execTransaction as a wallet_sendCalls leg");
    arrive(&mut sut, &method, &params);
    let blocked = sut.view().blocked.unwrap();
    assert_eq!(blocked.leg_index, Some(2));
    assert!(blocked.nested);
    assert_refused_as(&mut sut, "execTransaction", ParentReason::Member, &parent());
}

/// The wallet's own first-party requests are not exempt: Vela's multisig
/// flows never come this way, so nothing that does may act for a parent.
#[test]
fn a_first_party_request_is_held_to_the_same_rule() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("b: approveHash on a multisig on the list");
    arrive_as(&mut sut, &method, &params, true);
    assert_refused_as(&mut sut, "approveHash", ParentReason::Member, &parent());
}

/// The disclosure's PoC, through the machine: the two-message form never
/// reaches a sheet (one document per request, -32602), and the parent's
/// `SafeTx` alone is refused as a parent Safe's.
#[test]
fn the_appendix_poc_is_refused_both_ways() {
    let mut sut = boot(&[parent()]);
    let (method, params) =
        params_of("PoC (APPENDIX-PARENT): [zero Permit2, SafeTx for the parent]");
    let ops = arrive(&mut sut, &method, &params);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::SendResponse { payload: SignResponsePayload::Err { code, .. }, .. }
                if *code == CODE_INVALID_PARAMS
        )),
        "two documents are refused before any sheet: {ops:?}"
    );
    assert_eq!(sut.view().surface, SignSurface::Hidden);

    let safe_tx = params[1].clone();
    arrive(&mut sut, &method, &json!([signer(), safe_tx]));
    assert_refused_as(&mut sut, "SafeTx", ParentReason::Member, &parent());
}

#[test]
fn an_unknown_target_is_read_before_it_can_be_signed() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("unknown contract: read first");
    let ops = arrive(&mut sut, &method, &params);
    let asked = reads(&ops);
    assert_eq!(
        asked,
        vec![(
            chain(),
            unknown(),
            signer(),
            format!("{IS_OWNER}{:0>64}", signer().trim_start_matches("0x"))
        )],
        "isOwner(account) on the target, on the request's chain"
    );
    let view = sut.view();
    assert!(!view.confirm_gate_open, "the confirm waits for the read");
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Reading));
    assert!(view.blocked.is_none());
    // A tap from a stale frame waits too.
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::SignAndSubmit { .. } | Op::CheckBundlerFunding { .. }
        )),
        "nothing approved before the read: {ops:?}"
    );
}

#[test]
fn an_unknown_safe_this_account_owns_is_refused_after_the_read() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("unknown Safe, isOwner true: refused");
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &unknown(),
        SafeOwnerAnswer::Returned { data: word(1) },
    );
    assert_refused_as(&mut sut, "execTransaction", ParentReason::Owner, &unknown());
}

#[test]
fn an_unreadable_owner_check_refuses_it_never_allows() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("unknown contract: read first");
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &unknown(),
        SafeOwnerAnswer::Unreachable {
            detail: "every endpoint failed".to_owned(),
        },
    );
    assert_refused_as(&mut sut, "approveHash", ParentReason::Unchecked, &unknown());
}

#[test]
fn a_safe_this_account_does_not_own_opens_once_read() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("unknown contract: read first");
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &unknown(),
        SafeOwnerAnswer::Returned { data: word(0) },
    );
    let view = sut.view();
    assert!(view.blocked.is_none());
    assert!(view.confirm_gate_open, "read false: signable");
    // And the chokepoint lets the read it cleared through.
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::CheckBundlerFunding { .. })),
        "{ops:?}"
    );
    let ops = sut.resolve(SignShellResult::PreCheck { funding: None });
    assert!(
        sut.outstanding()
            .iter()
            .chain(ops.iter())
            .any(|op| matches!(op, Op::SignAndSubmit { .. })),
        "cleared at arrival, signed at submit"
    );
}

#[test]
fn a_read_for_another_request_changes_nothing() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("unknown contract: read first");
    arrive(&mut sut, &method, &params);
    let stale_safe = unknown();
    sut.resolve_matching(
        |op| matches!(op, Op::ReadSafeOwner { .. }),
        SignShellResult::SafeOwnerRead {
            id: "rid-0".to_owned(),
            chain_id: chain(),
            safe: stale_safe,
            answer: SafeOwnerAnswer::Returned { data: word(0) },
            witness: None,
            own_node: None,
        },
    );
    assert_eq!(sut.view().confirm_block, Some(ConfirmBlock::Reading));
}

/// The list can arrive after the request: the request on the sheet is
/// refused then, before the read answers.
#[test]
fn a_multisig_list_that_lands_late_still_refuses() {
    let mut sut = boot(&[]);
    let (method, params) = params_of("b: approveHash on a multisig on the list");
    let ops = arrive(&mut sut, &method, &params);
    assert_eq!(reads(&ops).len(), 1, "not on the list yet: read");
    sut.dispatch(Event::MultisigsChanged {
        addresses: vec![parent()],
    });
    assert_refused_as(&mut sut, "approveHash", ParentReason::Member, &parent());
}

/// The chokepoint reads the FINAL params: a rewrite that points a call at a
/// Safe nobody read is "couldn't check", never signed.
#[test]
fn a_rewrite_toward_an_unread_safe_is_refused_at_submit() {
    let mut sut = boot(&[]);
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &json!([{ "to": "0x1111111111111111111111111111111111111111", "value": "0x1" }]),
    );
    assert!(sut.view().confirm_gate_open, "an ordinary send opens");
    let (_, rewritten) = params_of("unknown contract: read first");
    sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts {
            params_override_json: Some(rewritten.to_string()),
            ..SignApproveOpts::default()
        },
    });
    let ops = sut.resolve(SignShellResult::PreCheck { funding: None });
    assert!(
        !sut.outstanding()
            .iter()
            .chain(ops.iter())
            .any(|op| matches!(op, Op::SignAndSubmit { .. })),
        "never signed"
    );
    let blocked = sut.view().blocked.expect("the sheet says why");
    assert_eq!(blocked.reason, Some(ParentReason::Unchecked));
    let closed = sut.dispatch(Event::SwipeDismissed);
    assert!(answered_parent_refusal(&closed), "{closed:?}");
}

/// A rewrite toward a multisig on the list, the same.
#[test]
fn a_rewrite_toward_a_multisig_is_refused_at_submit() {
    let mut sut = boot(&[parent()]);
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &json!([{ "to": "0x1111111111111111111111111111111111111111", "value": "0x1" }]),
    );
    let (_, rewritten) = params_of("a: execTransaction(swapOwner) on a multisig on the list, v=1");
    sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts {
            params_override_json: Some(rewritten.to_string()),
            ..SignApproveOpts::default()
        },
    });
    let ops = sut.resolve(SignShellResult::PreCheck { funding: None });
    assert!(!sut
        .outstanding()
        .iter()
        .chain(ops.iter())
        .any(|op| matches!(op, Op::SignAndSubmit { .. })));
    assert_eq!(
        sut.view().blocked.and_then(|b| b.reason),
        Some(ParentReason::Member)
    );
}

#[test]
fn an_ordinary_request_is_untouched() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("a token sent to a multisig");
    let ops = arrive(&mut sut, &method, &params);
    assert!(reads(&ops).is_empty());
    let view = sut.view();
    assert!(view.blocked.is_none());
    assert!(view.confirm_gate_open);
}

/// The answer the dApp gets names the call and the Safe, in the developer's
/// words; the person reads the catalog's.
#[test]
fn the_answer_names_the_call_and_the_safe() {
    let mut sut = boot(&[parent()]);
    let (method, params) = params_of("b: approveHash on a multisig on the list");
    arrive(&mut sut, &method, &params);
    let ops = sut.dispatch(Event::RejectTapped);
    let message = ops
        .iter()
        .find_map(|op| match op {
            Op::SendResponse {
                payload: SignResponsePayload::Err { message, kind, .. },
                ..
            } if *kind == SignErrorKind::ParentSafeBlocked => message.clone(),
            _ => None,
        })
        .expect("answered");
    assert!(message.contains("approveHash"), "{message}");
    assert!(message.contains(&parent()), "{message}");
    let words = vela_core::app::dapp_rpc::sign_error_words(
        SignErrorKind::ParentSafeBlocked,
        Some(&message),
    );
    assert!(words.contains("multisig"), "{words}");
}

#[test]
fn every_vector_verdict_round_trips() {
    for case in vector()["cases"].as_array().unwrap() {
        let verdict: ParentVerdict = serde_json::from_value(case["expect"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&verdict).unwrap(), case["expect"]);
    }
}

/// r18-10 (§32): a `SafeTx` under the domain of a Safe that is not on this
/// device's list was refused at once in the generic self-call words, before
/// the ownership read — though it IS a Safe this account owns. The parent
/// rule comes first: while the read is out nothing is refused or signable;
/// owned → the parent's words; not owned → still refused (a SafeTx is never
/// signed), in the self-call words.
#[test]
fn a_safe_tx_for_an_unlisted_safe_this_account_owns_is_refused_in_the_parents_words() {
    let mut sut = boot(&[]);
    let (method, params) =
        params_of("r18-10: unknown Safe, SafeTx, isOwner true: refused in the parent's words");
    let ops = arrive(&mut sut, &method, &params);
    assert_eq!(reads(&ops).len(), 1, "read first: {ops:?}");
    let view = sut.view();
    assert!(view.blocked.is_none(), "not refused before the read");
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Reading));
    assert!(!view.confirm_gate_open);
    answer_read(
        &mut sut,
        &unknown(),
        SafeOwnerAnswer::Returned { data: word(1) },
    );
    assert_refused_as(&mut sut, "SafeTx", ParentReason::Owner, &unknown());
}

#[test]
fn a_safe_tx_for_a_safe_this_account_does_not_own_is_still_refused() {
    let mut sut = boot(&[]);
    let (method, params) =
        params_of("r18-10: unknown Safe, SafeTx, isOwner true: refused in the parent's words");
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &unknown(),
        SafeOwnerAnswer::Returned { data: word(0) },
    );
    let view = sut.view();
    assert!(!view.confirm_gate_open, "a SafeTx is never signable");
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Refused));
    let blocked = view.blocked.expect("refused");
    assert_eq!(blocked.function, "SafeTx");
    assert_eq!(blocked.parent, None, "the self-call words");
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::SelfCallBlocked)
    );
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::SignAndSubmit { .. } | Op::CheckBundlerFunding { .. }
        )),
        "{ops:?}"
    );
}
