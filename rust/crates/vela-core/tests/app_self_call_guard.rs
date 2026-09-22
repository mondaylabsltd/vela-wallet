//! The self-call guard: a dApp may never ask this account to rewrite who
//! controls it (spec 081, FR-005/FR-006).
//!
//! The pure half is exercised directly; the machine half proves the request is
//! refused *and answered*, with the sheet left unsignable.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::self_call_guard::{detect_self_call, SelfCallFunction};
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignDappIdentity, SignErrorKind, SignOperation as Op,
    SignRequest, SignResponsePayload, SignSurface, CODE_INTERNAL,
};

type Sut = DomainDriver<SignRequest>;

const SAFE: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "0x3333333333333333333333333333333333333333";
const WP: &str = "wp-1";
const ORIGIN: &str = "https://dapp.example";
const NOW: f64 = 1_700_000_000_000.0;

/// 32-byte word holding a left-padded address.
fn word_addr(addr: &str) -> String {
    format!("{:0>64}", addr.trim_start_matches("0x"))
}

fn word_num(n: u64) -> String {
    format!("{n:064x}")
}

/// `addOwnerWithThreshold(owner, threshold)`.
fn add_owner_calldata() -> String {
    format!("0x0d582f13{}{}", word_addr(OTHER), word_num(1))
}

/// `enableModule(module)`.
fn enable_module_calldata() -> String {
    format!("0x610b5925{}", word_addr(OTHER))
}

/// One `multiSend` leg: operation(1) to(20) value(32) len(32) data.
fn leg(operation: u8, to: &str, data: &str) -> String {
    let body = data.trim_start_matches("0x");
    format!(
        "{:02x}{}{}{}{}",
        operation,
        to.trim_start_matches("0x"),
        word_num(0),
        word_num((body.len() / 2) as u64),
        body
    )
}

/// `multiSend(bytes)` wrapping the packed legs.
fn multi_send_calldata(legs: &[String]) -> String {
    let packed: String = legs.concat();
    format!(
        "0x8d80ff0a{}{}{}{}",
        word_num(32),
        word_num((packed.len() / 2) as u64),
        packed,
        // ABI tail padding to a 32-byte boundary
        "0".repeat((64 - (packed.len() % 64)) % 64)
    )
}

fn tx(to: &str, data: &str) -> Value {
    json!([{ "to": to, "data": data, "value": "0x0" }])
}

fn batch(calls: Vec<Value>) -> Value {
    json!([{ "calls": calls, "from": SAFE }])
}

// ---------------------------------------------------------------------------
// The pure rule
// ---------------------------------------------------------------------------

#[test]
fn every_control_changing_selector_is_blocked_on_a_self_call() {
    // selector, expected function
    let cases: Vec<(&str, SelfCallFunction)> = vec![
        ("0x0d582f13", SelfCallFunction::AddOwner),
        ("0xf8dc5dd9", SelfCallFunction::RemoveOwner),
        ("0xe318b52b", SelfCallFunction::SwapOwner),
        ("0x694e80c3", SelfCallFunction::ChangeThreshold),
        ("0x610b5925", SelfCallFunction::EnableModule),
        ("0xe009cfde", SelfCallFunction::DisableModule),
        ("0xe19a9dd9", SelfCallFunction::SetGuard),
        ("0xe068df37", SelfCallFunction::SetModuleGuard),
        ("0xf08a0323", SelfCallFunction::SetFallbackHandler),
        ("0xb63e800d", SelfCallFunction::Setup),
        ("0x6a761202", SelfCallFunction::ExecTransaction),
        ("0x468721a7", SelfCallFunction::ExecTransactionFromModule),
        ("0x5229073f", SelfCallFunction::ExecTransactionFromModule),
    ];
    for (selector, expected) in cases {
        let data = format!("{selector}{}", word_addr(OTHER));
        let block = detect_self_call("eth_sendTransaction", Some(&tx(SAFE, &data)), SAFE)
            .unwrap_or_else(|| panic!("{selector} must be blocked"));
        assert_eq!(block.function, expected, "{selector}");
        assert_eq!(block.leg_index, None);
        assert!(!block.nested);
    }
}

#[test]
fn the_same_call_to_another_contract_is_allowed() {
    // Only this account's own configuration is protected; a Safe someone else
    // owns is their business.
    assert!(detect_self_call(
        "eth_sendTransaction",
        Some(&tx(OTHER, &add_owner_calldata())),
        SAFE
    )
    .is_none());
}

#[test]
fn an_empty_self_call_is_allowed_because_the_fee_leg_uses_that_shape() {
    assert!(detect_self_call("eth_sendTransaction", Some(&tx(SAFE, "0x")), SAFE).is_none());
    // A plain value transfer to oneself, too.
    assert!(detect_self_call(
        "eth_sendTransaction",
        Some(&json!([{ "to": SAFE, "value": "0xde0b6b3a7640000" }])),
        SAFE
    )
    .is_none());
}

#[test]
fn an_ordinary_token_call_is_allowed() {
    // transfer(address,uint256) to a token contract — the everyday case.
    let data = format!("0xa9059cbb{}{}", word_addr(OTHER), word_num(1_000));
    assert!(detect_self_call("eth_sendTransaction", Some(&tx(OTHER, &data)), SAFE).is_none());
}

#[test]
fn a_batch_leg_is_blocked_and_names_its_position() {
    let calls = vec![
        json!({ "to": OTHER, "data": "0x" }),
        json!({ "to": SAFE, "data": enable_module_calldata() }),
    ];
    let block = detect_self_call("wallet_sendCalls", Some(&batch(calls)), SAFE).expect("blocked");
    assert_eq!(block.function, SelfCallFunction::EnableModule);
    assert_eq!(block.leg_index, Some(2), "1-based leg position");
}

#[test]
fn a_self_call_nested_in_multisend_is_blocked() {
    let inner = multi_send_calldata(&[
        leg(0, OTHER, "0x"),
        leg(0, SAFE, &add_owner_calldata()),
    ]);
    let block =
        detect_self_call("eth_sendTransaction", Some(&tx(OTHER, &inner)), SAFE).expect("blocked");
    assert_eq!(block.function, SelfCallFunction::AddOwner);
    assert!(block.nested);
}

#[test]
fn a_delegatecall_leg_is_blocked_whatever_it_targets() {
    // The Bybit primitive: a dApp cannot express `operation = 1` through
    // {to, value, data}, so hand-built calldata carrying one is hostile.
    let inner = multi_send_calldata(&[leg(1, OTHER, "0x12345678")]);
    let block =
        detect_self_call("eth_sendTransaction", Some(&tx(OTHER, &inner)), SAFE).expect("blocked");
    assert_eq!(block.function, SelfCallFunction::DelegateCall);
    assert!(block.nested);
}

#[test]
fn a_safe_tx_typed_data_request_is_blocked() {
    let typed = json!({
        "primaryType": "SafeTx",
        "domain": { "chainId": 1, "verifyingContract": SAFE },
        "message": { "to": OTHER, "value": "0", "data": "0x" }
    });
    let params = json!([SAFE, typed.to_string()]);
    let block =
        detect_self_call("eth_signTypedData_v4", Some(&params), SAFE).expect("SafeTx is blocked");
    assert_eq!(block.function, SelfCallFunction::SafeTxTypedData);
}

#[test]
fn ordinary_typed_data_is_allowed() {
    let typed = json!({
        "primaryType": "Permit",
        "domain": { "chainId": 1, "verifyingContract": OTHER },
        "message": { "value": "1" }
    });
    let params = json!([SAFE, typed.to_string()]);
    assert!(detect_self_call("eth_signTypedData_v4", Some(&params), SAFE).is_none());
}

#[test]
fn hostile_calldata_never_panics() {
    for data in ["0x", "0xzz", "0x0d582f13ff", "0x8d80ff0a", "0x6a761202beef"] {
        let _ = detect_self_call("eth_sendTransaction", Some(&tx(SAFE, data)), SAFE);
    }
}

// ---------------------------------------------------------------------------
// The machine: refused, answered, and left unsignable
// ---------------------------------------------------------------------------

fn boot() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 137],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: SAFE.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

fn arrive(sut: &mut Sut, method: &str, params: &Value) -> Vec<Op> {
    sut.dispatch(Event::RequestArrived {
        id: "rid-1".to_owned(),
        method: method.to_owned(),
        params_json: params.to_string(),
        origin: ORIGIN.to_owned(),
        transport_id: WP.to_owned(),
        dedicated_transport: false,
        per_request_chain: None,
        dapp: Some(SignDappIdentity {
            name: "app.example".to_owned(),
            url: Some(ORIGIN.to_owned()),
        }),
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
    })
}

#[test]
fn a_blocked_request_explains_itself_before_it_answers() {
    let mut sut = boot();
    let ops = arrive(
        &mut sut,
        "eth_sendTransaction",
        &tx(SAFE, &enable_module_calldata()),
    );

    // NOT answered yet: the window showing this sheet is the answer surface —
    // the extension worker closes it the moment the request settles — so an
    // immediate answer would take the explanation off the screen first.
    assert!(
        !ops.iter().any(|op| matches!(op, Op::SendResponse { .. })),
        "the answer waits for the person to close the sheet"
    );

    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet, "the sheet explains it");
    assert!(!view.confirm_gate_open, "there is nothing to confirm");
    let blocked = view.blocked.expect("the sheet carries the reason");
    assert_eq!(blocked.function, "enableModule");
    assert_eq!(blocked.leg_index, None);
    assert_eq!(
        view.error.map(|e| e.kind),
        Some(SignErrorKind::SelfCallBlocked)
    );

    // Closing it answers the dApp — and says the WALLET refused, not the
    // person, who was never offered the choice.
    let ops = sut.dispatch(Event::RejectTapped);
    assert!(
        answered_with_the_refusal(&ops),
        "an explicit reject answers with the refusal, never 4001"
    );
}

/// The event the SHELLS actually send when the sheet is closed.
///
/// This test exists because the one above did not catch a bug that bricked
/// signing on a phone. Android and iOS dismiss a sheet with `SwipeDismissed`
/// (web's close button is the only `RejectTapped` in the product), and
/// `dismiss()` deliberately sends no response — so a refused request was never
/// answered, the dApp waited forever, and every later request came back
/// "another request is open" until the app was killed. Found on the Xiaomi,
/// invisible to a suite that only ever pressed the button web uses.
#[test]
fn closing_a_blocked_sheet_the_way_a_phone_does_still_answers() {
    let mut sut = boot();
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &tx(SAFE, &enable_module_calldata()),
    );

    let ops = sut.dispatch(Event::SwipeDismissed);
    assert!(
        answered_with_the_refusal(&ops),
        "a swipe-dismissed refusal must answer too, or the transport never reopens"
    );

    // And the sheet is gone, so the next request can take the slot.
    assert_eq!(sut.view().surface, SignSurface::Hidden);
}

fn answered_with_the_refusal(ops: &[Op]) -> bool {
    ops.iter().any(|op| {
        matches!(
            op,
            Op::SendResponse { payload: SignResponsePayload::Err { code, kind, .. }, .. }
                if *code == CODE_INTERNAL && *kind == SignErrorKind::SelfCallBlocked
        )
    })
}

#[test]
fn a_blocked_request_cannot_be_signed_even_if_the_shell_asks() {
    let mut sut = boot();
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &tx(SAFE, &add_owner_calldata()),
    );
    assert!(!sut.view().confirm_gate_open);

    // A shell that ignores the gate gets the fail-closed answer from the
    // submit chokepoint instead of a signature.
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::SignAndSubmit { .. })),
        "nothing is ever signed for a refused request"
    );
}

#[test]
fn an_ordinary_request_still_opens_normally() {
    let mut sut = boot();
    arrive(
        &mut sut,
        "eth_sendTransaction",
        &json!([{ "to": OTHER, "data": "0x", "value": "0x1" }]),
    );
    let view = sut.view();
    assert_eq!(view.surface, SignSurface::Sheet);
    assert!(view.confirm_gate_open, "a normal request is signable");
    assert!(view.blocked.is_none());
}
