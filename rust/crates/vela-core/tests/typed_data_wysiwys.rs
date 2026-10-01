//! What you see is what you sign — typed data (audit 2026-10-01,
//! `docs/security/2026-10-01-eip712-wysiwys-audit.md`).
//!
//! The audit's probes, inverted: every reader takes the request's ONE
//! document, a request with two (in either order), the wrong order, one param
//! or an unknown method name is refused before a sheet opens, and what reaches
//! the sheet carries nothing but the canonical two params.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::approval_guard::{detect_approval, GuardApprovalKind, GuardBlockReason};
use vela_core::app::dapp_rpc::requested_address;
use vela_core::app::self_call_guard::detect_self_call;
use vela_core::app::sign_request::{
    extract_request_chain_id, Event, SignAccountRef, SignErrorKind, SignOperation as Op,
    SignRequest, SignResponsePayload, SignSurface, CODE_INVALID_PARAMS, CODE_UNAUTHORIZED,
};
use vela_core::eip712::hash_typed_data;
use vela_core::sign_message::original_hash;

type Sut = DomainDriver<SignRequest>;

const ACCOUNT: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
const OTHER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn mail() -> Value {
    json!({
        "types": {
            "EIP712Domain": [{"name": "name", "type": "string"}, {"name": "chainId", "type": "uint256"}],
            "Mail": [{"name": "contents", "type": "string"}]
        },
        "primaryType": "Mail",
        "domain": {"name": "Ether Mail", "chainId": 1},
        "message": {"contents": "Hello"}
    })
}

fn permit() -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"}, {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"}, {"name": "verifyingContract", "type": "address"}
            ],
            "Permit": [
                {"name": "owner", "type": "address"}, {"name": "spender", "type": "address"},
                {"name": "value", "type": "uint256"}, {"name": "nonce", "type": "uint256"},
                {"name": "deadline", "type": "uint256"}
            ]
        },
        "primaryType": "Permit",
        "domain": {"name": "USD Coin", "version": "2", "chainId": 1,
                   "verifyingContract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"},
        "message": {"owner": ACCOUNT, "spender": "0x000000000000000000000000000000000000dEaD",
                    "value": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
                    "nonce": "0", "deadline": "99999999999"}
    })
}

fn safe_tx() -> Value {
    json!({
        "types": {
            "EIP712Domain": [{"name": "chainId", "type": "uint256"}, {"name": "verifyingContract", "type": "address"}],
            "SafeTx": [
                {"name": "to", "type": "address"}, {"name": "value", "type": "uint256"},
                {"name": "data", "type": "bytes"}, {"name": "operation", "type": "uint8"},
                {"name": "safeTxGas", "type": "uint256"}, {"name": "baseGas", "type": "uint256"},
                {"name": "gasPrice", "type": "uint256"}, {"name": "gasToken", "type": "address"},
                {"name": "refundReceiver", "type": "address"}, {"name": "nonce", "type": "uint256"}
            ]
        },
        "primaryType": "SafeTx",
        "domain": {"chainId": 1, "verifyingContract": "0x1111111111111111111111111111111111111111"},
        "message": {"to": "0x000000000000000000000000000000000000dEaD", "value": "0", "data": "0x",
                    "operation": 1, "safeTxGas": "0", "baseGas": "0", "gasPrice": "0",
                    "gasToken": "0x0000000000000000000000000000000000000000",
                    "refundReceiver": "0x0000000000000000000000000000000000000000", "nonce": "0"}
    })
}

fn permit2_transfer(batch: bool) -> Value {
    let permitted =
        json!({"token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "amount": "1000000000"});
    json!({
        "types": {
            "EIP712Domain": [{"name": "name", "type": "string"}, {"name": "chainId", "type": "uint256"},
                             {"name": "verifyingContract", "type": "address"}],
            (if batch { "PermitBatchTransferFrom" } else { "PermitTransferFrom" }): [
                {"name": "permitted", "type": if batch { "TokenPermissions[]" } else { "TokenPermissions" }},
                {"name": "spender", "type": "address"},
                {"name": "nonce", "type": "uint256"}, {"name": "deadline", "type": "uint256"}
            ],
            "TokenPermissions": [{"name": "token", "type": "address"}, {"name": "amount", "type": "uint256"}]
        },
        "primaryType": if batch { "PermitBatchTransferFrom" } else { "PermitTransferFrom" },
        "domain": {"name": "Permit2", "chainId": 1,
                   "verifyingContract": "0x000000000022D473030F116dDEE9F6B43aC78BA3"},
        "message": {"permitted": if batch { json!([permitted]) } else { permitted },
                    "spender": "0x000000000000000000000000000000000000dEaD",
                    "nonce": "0", "deadline": "99999999999"}
    })
}

fn digest(doc: &Value) -> Vec<u8> {
    hash_typed_data(&doc.to_string()).expect("hashes")
}

// --- the pure readers ----------------------------------------------------------

#[test]
fn two_documents_are_signed_by_no_one() {
    // The audit's shape A (v4) and B (legacy): no digest at all.
    assert_eq!(
        original_hash(
            "eth_signTypedData_v4",
            &json!([mail(), permit()]).to_string()
        ),
        None
    );
    for method in ["eth_signTypedData", "eth_signTypedData_v1"] {
        assert_eq!(
            original_hash(method, &json!([permit(), mail()]).to_string()),
            None,
            "{method}"
        );
    }
    // Neither is a reading the guards or the chain pick can take.
    assert!(detect_approval("eth_signTypedData", Some(&json!([permit(), mail()]))).is_none());
    assert_eq!(
        extract_request_chain_id("eth_signTypedData_v4", &json!([mail(), permit()])),
        None
    );
    assert_eq!(
        requested_address("eth_signTypedData_v4", &json!([mail(), permit()])),
        None
    );
}

#[test]
fn a_well_formed_request_signs_its_one_document_in_every_method() {
    for (method, params) in [
        ("eth_signTypedData_v4", json!([ACCOUNT, permit()])),
        (
            "eth_signTypedData_v3",
            json!([ACCOUNT, permit().to_string()]),
        ),
        ("eth_signTypedData", json!([permit(), ACCOUNT])),
        (
            "eth_signTypedData_v1",
            json!([permit().to_string(), ACCOUNT]),
        ),
    ] {
        assert_eq!(
            original_hash(method, &params.to_string()),
            Some(digest(&permit())),
            "{method}"
        );
        assert_eq!(
            requested_address(method, &params).as_deref(),
            Some(ACCOUNT),
            "{method}"
        );
        assert_eq!(
            extract_request_chain_id(method, &params),
            Some(1),
            "{method}"
        );
        // The legacy order used to hide the Permit from the approval guard.
        let detected = detect_approval(method, Some(&params)).expect(method);
        assert_eq!(detected.kind, GuardApprovalKind::Erc2612Permit, "{method}");
    }
}

#[test]
fn a_safe_tx_anywhere_is_refused() {
    for params in [
        json!([mail(), safe_tx()]),
        json!([ACCOUNT, safe_tx()]),
        json!([safe_tx(), ACCOUNT]),
    ] {
        assert!(
            detect_self_call("eth_signTypedData_v4", Some(&params), ACCOUNT).is_some(),
            "{params}"
        );
    }
    assert!(detect_self_call(
        "eth_signTypedData_v4",
        Some(&json!([ACCOUNT, mail()])),
        ACCOUNT
    )
    .is_none());
}

#[test]
fn one_param_the_wrong_order_and_other_names_sign_nothing() {
    assert_eq!(
        original_hash("eth_signTypedData_v4", &json!([permit()]).to_string()),
        None
    );
    assert_eq!(
        original_hash(
            "eth_signTypedData_v4",
            &json!([permit(), ACCOUNT]).to_string()
        ),
        None
    );
    assert_eq!(
        original_hash("eth_signTypedData", &json!([ACCOUNT, permit()]).to_string()),
        None
    );
    for method in ["eth_signTypedData_v2", "x_signTypedData_anything"] {
        assert_eq!(
            original_hash(method, &json!([ACCOUNT, permit()]).to_string()),
            None,
            "{method}"
        );
    }
}

#[test]
fn permit2_signature_transfer_is_flagged() {
    for (batch, kind) in [
        (false, GuardApprovalKind::Permit2Single),
        (true, GuardApprovalKind::Permit2Batch),
    ] {
        let params = json!([ACCOUNT, permit2_transfer(batch)]);
        let detected = detect_approval("eth_signTypedData_v4", Some(&params)).expect("detected");
        assert_eq!(detected.kind, kind);
        assert_eq!(
            detected.block_reason,
            Some(GuardBlockReason::OffChainPermit)
        );
        assert!(!detected.editable, "signed verbatim, never capped");
        assert_eq!(
            detected.spender,
            "0x000000000000000000000000000000000000dead"
        );
    }
}

// --- the machine: refused before any sheet; the sheet holds one document ------

fn boot() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 100],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: ACCOUNT.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

fn arrive(id: &str, method: &str, params: &Value, granted: Option<&str>) -> Event {
    Event::RequestArrived {
        id: id.to_owned(),
        method: method.to_owned(),
        params_json: params.to_string(),
        origin: "https://dapp.example".to_owned(),
        transport_id: "ext-1".to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(1),
        dapp: None,
        granted_address: granted.map(str::to_owned),
        requested_address: None,
        request_ts_ms: None,
        now_ms: 1_700_000_000_000.0,
    }
}

fn refusal(ops: &[Op]) -> Option<(i32, SignErrorKind, Option<String>)> {
    ops.iter().find_map(|op| match op {
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
    })
}

#[test]
fn the_malicious_shapes_are_refused_before_the_sheet() {
    for (method, params) in [
        ("eth_signTypedData_v4", json!([mail(), permit()])),
        ("eth_signTypedData", json!([permit(), mail()])),
        ("eth_signTypedData_v1", json!([permit(), mail()])),
        ("eth_signTypedData_v4", json!([permit()])),
        ("eth_signTypedData_v2", json!([ACCOUNT, permit()])),
    ] {
        let mut sut = boot();
        let ops = sut.dispatch(arrive("r1", method, &params, Some(ACCOUNT)));
        let (code, kind, message) =
            refusal(&ops).unwrap_or_else(|| panic!("{method} {params}: not refused"));
        assert_eq!(
            (code, kind),
            (CODE_INVALID_PARAMS, SignErrorKind::InvalidParams),
            "{method}"
        );
        assert!(message.is_some_and(|m| !m.is_empty()), "{method}: says why");
        assert_eq!(
            sut.view().surface,
            SignSurface::Hidden,
            "{method}: never reached the sheet"
        );
        assert!(sut.view().request.is_none(), "{method}");
    }
}

#[test]
fn a_document_for_another_account_is_refused_4100() {
    let mut sut = boot();
    let ops = sut.dispatch(arrive(
        "r2",
        "eth_signTypedData_v4",
        &json!([OTHER, permit()]),
        Some(ACCOUNT),
    ));
    let (code, kind, _) = refusal(&ops).expect("refused");
    assert_eq!(
        (code, kind),
        (CODE_UNAUTHORIZED, SignErrorKind::UnauthorizedAccount)
    );
    assert!(sut.view().request.is_none());
}

#[test]
fn the_sheet_holds_only_the_canonical_params() {
    // A legacy request sent with its document as an object: the sheet and
    // every later reader get `[documentJson, account]`, the one document.
    let mut sut = boot();
    let ops = sut.dispatch(arrive(
        "r3",
        "eth_signTypedData",
        &json!([permit(), ACCOUNT]),
        Some(ACCOUNT),
    ));
    assert!(refusal(&ops).is_none(), "{ops:?}");
    let request = sut.view().request.expect("on the sheet");
    let params: Value = serde_json::from_str(&request.params_json).expect("json");
    assert_eq!(params, json!([permit().to_string(), ACCOUNT]));
    assert_eq!(
        original_hash("eth_signTypedData", &request.params_json),
        Some(digest(&permit()))
    );
}
