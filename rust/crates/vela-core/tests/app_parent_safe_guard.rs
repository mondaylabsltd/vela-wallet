//! VELA-2026-002 (the 0.9.9 hotfix) — the parent-Safe guard: a site may never
//! make this account act for a Safe it owns. Reported 2026-09-29 (disclosure
//! appendix APPENDIX-PARENT); backported from feat/multisig-account §31
//! without the multisig list v0.9.8 does not have.
//!
//! The vector `vectors/rules/parent-safe-guard.json` is the contract every
//! platform replays; this is its first reader. The machine half proves the
//! request is refused before it can be signed — after an `isOwner` read of
//! every Safe it would act for, and again at the submit chokepoint — that a
//! read nobody could make refuses, and that the refusal is answered.
//!
//! 0.9.10 (backported from feat/multisig-account §41): a site's address not
//! written as `0x` + 40 hex is refused at arrival (-32602), every guard reads
//! an address as the encoders do, and a `SafeOp` for an owned Safe is refused
//! like `SafeTx` / `SafeMessage` (Safe v1.5.0 counts the wrap as approval).
#![cfg(feature = "crux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::parent_safe_guard::{
    check, check_json, CheckInput, CheckVerdict, ParentReason, SafeOwnerAnswer, IS_OWNER,
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
        "hotfix exec-swap",
        "hotfix exec-add",
        "hotfix exec-thr",
        "hotfix approveHash",
        "hotfix MultiSend-nested",
        "hotfix SafeTx",
        "hotfix SafeMessage",
        "hotfix exec-swap: unknown Safe, not owner, allowed",
        "hotfix approveHash: isOwner unreadable",
        // 0.9.10
        "0.9.10 non-canonical: approveHash with a bare-hex `to`",
        "0.9.10 non-canonical: the takeover execTransaction (v=1) with a bare-hex `to`",
        "0.9.10 non-canonical: a wallet_sendCalls leg with a bare-hex `to`",
        "0.9.10 non-canonical: SafeMessage under a bare verifyingContract",
        "0.9.10 non-canonical: SafeTx under an under-width verifyingContract",
        "0.9.10 at arrival: a bare-hex `to` is refused -32602",
        "0.9.10 at arrival: a bare verifyingContract is refused -32602",
        "0.9.10 at arrival: an under-width verifyingContract is refused -32602",
        "0.9.10 v1.5.0: SafeMessage under an owned Safe v1.5.0",
        "0.9.10 v1.5.0: SafeOp for an owned Safe v1.5.0",
        "0.9.10 bare execTransaction: bare-hex `to`, isOwner true: refused",
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

fn boot() -> Sut {
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
        SignShellResult::SafeOwnerRead {
            id: "rid-1".to_owned(),
            chain_id: chain(),
            safe: safe.clone(),
            answer,
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

/// Arrives, is read (`isOwner(account)` on the Safe), reads true.
fn arrive_owned(sut: &mut Sut, name: &str, first_party: bool) {
    let (method, params) = params_of(name);
    let ops = arrive_as(sut, &method, &params, first_party);
    assert_eq!(
        reads(&ops),
        vec![(
            chain(),
            parent(),
            signer(),
            format!("{IS_OWNER}{:0>64}", signer().trim_start_matches("0x"))
        )],
        "the Safe is read before anything is decided"
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::SendResponse { .. })),
        "the answer waits for the person to close the sheet"
    );
    assert_eq!(sut.view().confirm_block, Some(ConfirmBlock::Reading));
    answer_read(sut, &parent(), SafeOwnerAnswer::Returned { data: word(1) });
}

#[test]
fn a_takeover_exec_transaction_on_an_owned_safe_is_refused() {
    let mut sut = boot();
    arrive_owned(
        &mut sut,
        "a: execTransaction(swapOwner) on a Safe this account owns, v=1",
        false,
    );
    assert_refused_as(&mut sut, "execTransaction", ParentReason::Owner, &parent());
}

#[test]
fn approve_hash_on_an_owned_safe_is_refused() {
    let mut sut = boot();
    arrive_owned(
        &mut sut,
        "b: approveHash on a Safe this account owns",
        false,
    );
    assert_refused_as(&mut sut, "approveHash", ParentReason::Owner, &parent());
}

#[test]
fn a_safe_message_under_an_owned_safes_domain_is_refused_not_signed_anyway() {
    let mut sut = boot();
    arrive_owned(
        &mut sut,
        "c: SafeMessage under an owned Safe's domain",
        false,
    );
    assert_refused_as(&mut sut, "SafeMessage", ParentReason::Owner, &parent());
}

/// A `SafeTx` was already refused by the self-call guard (spec 081), for any
/// domain, the moment it arrived.
#[test]
fn a_safe_tx_is_refused_at_once() {
    let mut sut = boot();
    let (method, params) = params_of("c: SafeTx under an owned Safe's domain");
    arrive(&mut sut, &method, &params);
    let view = sut.view();
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Refused));
    assert_eq!(view.blocked.unwrap().function, "SafeTx");
}

#[test]
fn a_multisend_hiding_an_approval_is_refused() {
    let mut sut = boot();
    arrive_owned(
        &mut sut,
        "MultiSend: the takeover execTransaction as a wallet_sendCalls leg",
        false,
    );
    let blocked = sut.view().blocked.unwrap();
    assert_eq!(blocked.leg_index, Some(2));
    assert!(blocked.nested);
    assert_refused_as(&mut sut, "execTransaction", ParentReason::Owner, &parent());
}

/// The wallet's own first-party requests are not exempt: v0.9.8 has no flow
/// of its own that acts for another Safe, so nothing that comes this way may.
#[test]
fn a_first_party_request_is_held_to_the_same_rule() {
    let mut sut = boot();
    arrive_owned(&mut sut, "b: approveHash on a Safe this account owns", true);
    assert_refused_as(&mut sut, "approveHash", ParentReason::Owner, &parent());
}

/// Every named hotfix shape, through the machine: read, owned, refused.
#[test]
fn every_hotfix_shape_is_refused_once_read_owned() {
    for (short, function) in [
        ("exec-swap", "execTransaction"),
        ("exec-add", "execTransaction"),
        ("exec-thr", "execTransaction"),
        ("approveHash", "approveHash"),
        ("MultiSend-nested", "approveHash"),
        ("SafeMessage", "SafeMessage"),
    ] {
        let mut sut = boot();
        let (method, params) = params_of(&format!("hotfix {short}: isOwner true, refused"));
        let ops = arrive(&mut sut, &method, &params);
        assert_eq!(reads(&ops).len(), 1, "{short}: read first");
        answer_read(
            &mut sut,
            &unknown(),
            SafeOwnerAnswer::Returned { data: word(1) },
        );
        assert_refused_as(&mut sut, function, ParentReason::Owner, &unknown());
    }
}

/// Every named hotfix shape whose read cannot be made: refused, never allowed.
#[test]
fn every_hotfix_shape_is_refused_when_the_read_fails() {
    for (short, function) in [
        ("exec-swap", "execTransaction"),
        ("approveHash", "approveHash"),
        ("MultiSend-nested", "approveHash"),
        ("SafeMessage", "SafeMessage"),
    ] {
        let mut sut = boot();
        let (method, params) = params_of(&format!(
            "hotfix {short}: isOwner unreadable, refused couldn't check"
        ));
        arrive(&mut sut, &method, &params);
        answer_read(
            &mut sut,
            &unknown(),
            SafeOwnerAnswer::Unreachable {
                detail: "offline".to_owned(),
            },
        );
        assert_refused_as(&mut sut, function, ParentReason::Unchecked, &unknown());
    }
}

/// The same shapes on a Safe this account does not own open as before
/// (the `SafeTx` aside: the self-call guard refuses every one).
#[test]
fn every_hotfix_shape_opens_on_a_safe_not_owned() {
    for short in [
        "exec-swap",
        "exec-add",
        "exec-thr",
        "approveHash",
        "MultiSend-nested",
        "SafeMessage",
    ] {
        let mut sut = boot();
        let (method, params) =
            params_of(&format!("hotfix {short}: unknown Safe, not owner, allowed"));
        arrive(&mut sut, &method, &params);
        answer_read(
            &mut sut,
            &unknown(),
            SafeOwnerAnswer::Returned { data: word(0) },
        );
        let view = sut.view();
        assert!(view.blocked.is_none(), "{short}");
        assert_ne!(view.confirm_block, Some(ConfirmBlock::Reading), "{short}");
        assert_ne!(view.confirm_block, Some(ConfirmBlock::Refused), "{short}");
    }
}

/// The disclosure's PoC, through the machine: the two-message form never
/// reaches a sheet (one document per request, -32602), and the parent's
/// `SafeTx` alone is refused as a parent Safe's.
#[test]
fn the_appendix_poc_is_refused_both_ways() {
    let mut sut = boot();
    let (method, params) =
        params_of("PoC (APPENDIX-PARENT): [zero Permit2, SafeTx for the owned parent]");
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

    // The SafeTx alone is the self-call guard's at once (spec 081).
    let safe_tx = params[1].clone();
    arrive(&mut sut, &method, &json!([signer(), safe_tx]));
    let view = sut.view();
    assert_eq!(view.confirm_block, Some(ConfirmBlock::Refused));
    assert_eq!(view.blocked.unwrap().function, "SafeTx");
}

/// 0.9.10: every arrival vector's refusal, through the machine — a
/// non-canonical `to`, batch leg or `verifyingContract` is answered -32602
/// before any sheet and before any read, in the vector's words; a canonical
/// one reaches the guard (read first, then refused).
#[test]
fn a_non_canonical_address_is_refused_at_arrival_by_the_machine() {
    let v = vector();
    let arrivals: Vec<&Value> = v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["input"]["at_arrival"] == json!(true))
        .collect();
    assert!(arrivals.len() >= 8, "the arrival cases");
    for case in arrivals {
        let name = case["name"].as_str().unwrap();
        let mut sut = boot();
        let method = case["input"]["method"].as_str().unwrap();
        let ops = arrive(&mut sut, method, &case["input"]["params"]);
        let answered = ops.iter().find_map(|op| match op {
            Op::SendResponse {
                payload: SignResponsePayload::Err { code, message, .. },
                ..
            } => Some((*code, message.clone())),
            _ => None,
        });
        if case["expect"]["verdict"] == "invalid" {
            let (code, message) = answered.unwrap_or_else(|| panic!("{name}: {ops:?}"));
            assert_eq!(code, CODE_INVALID_PARAMS, "{name}");
            assert_eq!(
                message.as_deref(),
                case["expect"]["message"].as_str(),
                "{name}"
            );
            assert!(reads(&ops).is_empty(), "{name}: nothing is read");
            assert_eq!(sut.view().surface, SignSurface::Hidden, "{name}");
        } else {
            assert!(answered.is_none(), "{name}: {ops:?}");
            assert_eq!(
                reads(&ops)
                    .into_iter()
                    .map(|(_, safe, _, _)| safe)
                    .collect::<Vec<_>>(),
                vec![parent()],
                "{name}: the guard reads the Safe"
            );
            let target = case["expect"]["target"].clone();
            answer_read(
                &mut sut,
                &parent(),
                SafeOwnerAnswer::Returned { data: word(1) },
            );
            assert_refused_as(
                &mut sut,
                target["function"].as_str().unwrap(),
                ParentReason::Owner,
                &parent(),
            );
        }
    }
}

const SAFE_OP_V150: &str = "0.9.10 v1.5.0: SafeOp for an owned Safe v1.5.0 (its `safe`), \
                            under the 4337 module's domain: refused";

/// 0.9.10: a `SafeMessage` or `SafeOp` for a Safe this account owns — under
/// a Safe v1.5.0, either is the account's approval — opens refused once the
/// read says owned; unreadable, refused "couldn't check"; not owned, as before.
#[test]
fn a_safe_message_or_safe_op_for_an_owned_v150_safe_is_refused_by_the_machine() {
    for (name, function) in [
        (
            "0.9.10 v1.5.0: SafeMessage under an owned Safe v1.5.0's domain, isOwner true: refused",
            "SafeMessage",
        ),
        (SAFE_OP_V150, "SafeOp"),
        (
            "0.9.10 c: SafeOp under an owned Safe's own domain",
            "SafeOp",
        ),
        (
            "0.9.10 non-canonical: SafeOp whose `safe` is written bare",
            "SafeOp",
        ),
    ] {
        let mut sut = boot();
        let (method, params) = params_of(name);
        let ops = arrive(&mut sut, &method, &params);
        assert!(
            reads(&ops).iter().any(|r| r.1 == parent()),
            "{name}: {ops:?}"
        );
        answer_read(
            &mut sut,
            &parent(),
            SafeOwnerAnswer::Returned { data: word(1) },
        );
        assert_refused_as(&mut sut, function, ParentReason::Owner, &parent());
    }

    let (method, params) = params_of(SAFE_OP_V150);
    let mut sut = boot();
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &parent(),
        SafeOwnerAnswer::Unreachable {
            detail: "offline".to_owned(),
        },
    );
    assert_refused_as(&mut sut, "SafeOp", ParentReason::Unchecked, &parent());

    let mut sut = boot();
    arrive(&mut sut, &method, &params);
    answer_read(
        &mut sut,
        &parent(),
        SafeOwnerAnswer::Returned { data: word(0) },
    );
    let view = sut.view();
    assert!(view.blocked.is_none(), "not owned: as before");
    assert_ne!(view.confirm_block, Some(ConfirmBlock::Refused));
    assert_ne!(view.confirm_block, Some(ConfirmBlock::Reading));
}

/// 0.9.10: the submit chokepoint reads a bare-hex `to` the way the encoder
/// signs it — a rewrite toward the bare-hex takeover is refused, never signed.
#[test]
fn a_rewrite_toward_a_bare_hex_takeover_is_refused_at_submit() {
    let mut sut = boot();
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &json!([{ "to": "0x1111111111111111111111111111111111111111", "value": "0x1" }]),
    );
    let (_, rewritten) =
        params_of("0.9.10 non-canonical: the takeover execTransaction (v=1) with a bare-hex `to`");
    assert_eq!(
        rewritten[0]["to"].as_str(),
        Some(parent().trim_start_matches("0x"))
    );
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
    let blocked = sut.view().blocked.expect("the sheet says why");
    assert_eq!(blocked.reason, Some(ParentReason::Unchecked));
    assert_eq!(blocked.parent.as_deref(), Some(parent().as_str()));
}

#[test]
fn an_unknown_target_is_read_before_it_can_be_signed() {
    let mut sut = boot();
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
    let mut sut = boot();
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
    let mut sut = boot();
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
    let mut sut = boot();
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
    let mut sut = boot();
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
        },
    );
    assert_eq!(sut.view().confirm_block, Some(ConfirmBlock::Reading));
}

/// The chokepoint reads the FINAL params: a rewrite that points a call at a
/// Safe nobody read is "couldn't check", never signed.
#[test]
fn a_rewrite_toward_an_unread_safe_is_refused_at_submit() {
    let mut sut = boot();
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

/// A rewrite toward an execTransaction on a Safe nobody read, the same.
#[test]
fn a_rewrite_toward_a_takeover_is_refused_at_submit() {
    let mut sut = boot();
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &json!([{ "to": "0x1111111111111111111111111111111111111111", "value": "0x1" }]),
    );
    let (_, rewritten) =
        params_of("a: execTransaction(swapOwner) on a Safe this account owns, v=1");
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
        Some(ParentReason::Unchecked)
    );
}

#[test]
fn an_ordinary_request_is_untouched() {
    let mut sut = boot();
    let (method, params) = params_of("a token sent to a Safe");
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
    let mut sut = boot();
    arrive_owned(
        &mut sut,
        "b: approveHash on a Safe this account owns",
        false,
    );
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
    assert!(words.contains("Safe this account owns"), "{words}");
}

#[test]
fn every_vector_verdict_round_trips() {
    for case in vector()["cases"].as_array().unwrap() {
        let verdict: CheckVerdict = serde_json::from_value(case["expect"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&verdict).unwrap(), case["expect"]);
    }
}
