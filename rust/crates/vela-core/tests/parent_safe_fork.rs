//! VELA-2026-002 (the 0.9.9 hotfix) on a Gnosis fork — the APPENDIX-PARENT
//! takeover, for real, and Vela refusing it.
//!
//! A Safe v1.4.1 made outside Vela lists one owner: the "Vela account"
//! (`OWNER`, impersonated on anvil — the call it would make is the same call
//! from the same `msg.sender`). A site connected to that account asks for
//! `safe.execTransaction(safe, 0, swapOwner(0x1, OWNER, ATTACKER), …, sig)`
//! with the "approved by msg.sender" signature (`r = OWNER, v = 1`).
//!
//! 1. Sent straight from the owner's address, that one call hands the Safe
//!    to the attacker (proved on a snapshot, then reverted).
//! 2. The same request through Vela 0.9.9's sign machine: the machine asks
//!    the shell to read `isOwner(OWNER)` on the Safe; this file answers it
//!    with a real `eth_call` to the fork; the chain says `true`, and the
//!    request is refused — no submit, no pre-check — and answered -32603
//!    `parent_safe_blocked`. The Safe's owners are unchanged.
//! 3. A normal dApp transaction (a plain transfer, and the same
//!    `execTransaction` shape aimed at a Safe this account does NOT own)
//!    goes on to the submit pipeline as before.
//!
//! 0.9.10 (the second test): the same takeover with the parent's address
//! written without its `0x`. The "Vela account" is now a Safe shaped like
//! one (the 4337 module enabled and its fallback handler) and the parent's
//! only owner. Vela's own encoder (`tx_request::calls_of` →
//! `user_op::build_execute_call_data`) reads the bare form as the parent, and
//! the EntryPoint running that `callData` hands the parent to the attacker —
//! what 0.9.9's guard, which read only `0x` + 40 hex, let through (proved on
//! a snapshot, then reverted). Through 0.9.10 the bare form, `0X` and a bare
//! batch leg are answered -32602 at arrival, before any read or sheet; the
//! `0x` form is still read and refused; a plain transfer still reaches the
//! submit and lands.
//!
//! Ignored by default; run with a fork up:
//!
//! ```sh
//! anvil --fork-url https://rpc.gnosischain.com --chain-id 100 --port 9461 &
//! VELA_PARENT_FORK=http://127.0.0.1:9461 \
//!   cargo test -p vela-core --features crux,dev-fixtures --test parent_safe_fork -- --ignored --nocapture --test-threads=1
//! ```
#![cfg(feature = "crux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use alloy_dyn_abi::DynSolValue;
use alloy_primitives::{hex, keccak256, Address, U256};
use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::parent_safe_guard::{ParentReason, SafeOwnerAnswer};
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignErrorKind, SignOperation as Op, SignRequest,
    SignResponsePayload, SignShellResult, SignSurface, CODE_INTERNAL, CODE_INVALID_PARAMS,
};

type Sut = DomainDriver<SignRequest>;

const CHAIN: u32 = 100;
/// Safe v1.4.1 on Gnosis: the proxy factory, the L2 singleton, the
/// compatibility fallback handler.
const FACTORY: &str = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67";
const SINGLETON: &str = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762";
const FALLBACK: &str = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99";
/// The Vela account (an owner of the outside Safe), and the attacker.
const OWNER: &str = "0x00000000000000000000000000000000000a11ce";
const ATTACKER: &str = "0x00000000000000000000000000000000000bad00";
const SENTINEL: &str = "0x0000000000000000000000000000000000000001";

fn fork() -> String {
    std::env::var("VELA_PARENT_FORK").unwrap_or_else(|_| "http://127.0.0.1:9461".to_owned())
}

/// One JSON-RPC call to `url` over a plain socket: the whole envelope.
fn rpc_at(url: &str, method: &str, params: &Value) -> Result<Value, String> {
    let host = url.strip_prefix("http://").ok_or("not http")?;
    let host = host.trim_end_matches('/');
    let mut s = TcpStream::connect(host).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let req = format!(
        "POST / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (_, payload) = text.split_once("\r\n\r\n").ok_or("no head")?;
    serde_json::from_str(payload).map_err(|e| format!("{e}: {payload}"))
}

fn rpc(method: &str, params: &Value) -> Value {
    let envelope = rpc_at(&fork(), method, params).unwrap_or_else(|e| panic!("{method}: {e}"));
    if let Some(error) = envelope.get("error") {
        panic!("{method}: {error}");
    }
    envelope["result"].clone()
}

fn addr(a: &str) -> DynSolValue {
    DynSolValue::Address(a.parse::<Address>().unwrap())
}

fn uint(n: u64) -> DynSolValue {
    DynSolValue::Uint(U256::from(n), 256)
}

fn bytes(hex_str: &str) -> DynSolValue {
    DynSolValue::Bytes(hex::decode(hex_str.trim_start_matches("0x")).unwrap())
}

/// `selector(signature) ‖ abi(args)`, as `0x…`.
fn calldata(signature: &str, args: Vec<DynSolValue>) -> String {
    let mut out = keccak256(signature.as_bytes())[..4].to_vec();
    out.extend(DynSolValue::Tuple(args).abi_encode_params());
    format!("0x{}", hex::encode(out))
}

/// A transaction from `from` (impersonated), mined; the receipt.
fn send_from(from: &str, to: &str, data: &str, value: &str) -> Value {
    rpc("anvil_impersonateAccount", &json!([from]));
    let hash = rpc(
        "eth_sendTransaction",
        &json!([{ "from": from, "to": to, "data": data, "value": value, "gas": "0x7a1200" }]),
    );
    let mut receipt = Value::Null;
    for _ in 0..100 {
        receipt = rpc("eth_getTransactionReceipt", &json!([hash]));
        if !receipt.is_null() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    rpc("anvil_stopImpersonatingAccount", &json!([from]));
    receipt
}

fn call(to: &str, data: &str) -> String {
    rpc("eth_call", &json!([{ "to": to, "data": data }, "latest"]))
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// `getOwners()` of `safe`, lowercase.
fn owners(safe: &str) -> Vec<String> {
    let raw = call(safe, &calldata("getOwners()", vec![]));
    let body = hex::decode(raw.trim_start_matches("0x")).unwrap();
    // offset(32) ‖ length(32) ‖ words
    let n = usize::try_from(U256::from_be_slice(&body[32..64])).unwrap();
    (0..n)
        .map(|i| {
            let word = &body[64 + i * 32..96 + i * 32];
            format!("0x{}", hex::encode(&word[12..]))
        })
        .collect()
}

/// A 1-of-1 Safe v1.4.1 whose only owner is `owner`, made through the
/// factory as anyone makes one; its address.
fn make_safe(owner: &str, salt: u64) -> String {
    let setup = calldata(
        "setup(address[],uint256,address,bytes,address,address,uint256,address)",
        vec![
            DynSolValue::Array(vec![addr(owner)]),
            uint(1),
            addr("0x0000000000000000000000000000000000000000"),
            bytes("0x"),
            addr(FALLBACK),
            addr("0x0000000000000000000000000000000000000000"),
            uint(0),
            addr("0x0000000000000000000000000000000000000000"),
        ],
    );
    let create = calldata(
        "createProxyWithNonce(address,bytes,uint256)",
        vec![addr(SINGLETON), bytes(&setup), uint(salt)],
    );
    let predicted = call(FACTORY, &create);
    let safe = format!(
        "0x{}",
        predicted.trim_start_matches("0x")[24..64].to_ascii_lowercase()
    );
    // Anyone may make it; the funded account pays.
    let receipt = send_from(OWNER, FACTORY, &create, "0x0");
    assert_eq!(receipt["status"], "0x1", "the Safe is made: {receipt}");
    assert_eq!(owners(&safe), vec![owner.to_ascii_lowercase()]);
    safe
}

/// The attack page's call: `safe.execTransaction(safe, 0,
/// swapOwner(0x1, owner, attacker), CALL, 0, 0, 0, 0, 0, sig)` with
/// `sig = r: owner, s: 0, v: 1` — "approved by msg.sender".
fn takeover_data(safe: &str, owner: &str) -> String {
    let swap = calldata(
        "swapOwner(address,address,address)",
        vec![addr(SENTINEL), addr(owner), addr(ATTACKER)],
    );
    let sig = format!(
        "0x{:0>64}{:0>64}01",
        owner.trim_start_matches("0x").to_ascii_lowercase(),
        ""
    );
    calldata(
        "execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
        vec![
            addr(safe),
            uint(0),
            bytes(&swap),
            DynSolValue::Uint(U256::ZERO, 8),
            uint(0),
            uint(0),
            uint(0),
            addr("0x0000000000000000000000000000000000000000"),
            addr("0x0000000000000000000000000000000000000000"),
            bytes(&sig),
        ],
    )
}

fn boot(signer: &str) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, CHAIN],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: signer.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

fn arrive(sut: &mut Sut, signer: &str, params: &Value) -> Vec<Op> {
    sut.dispatch(Event::RequestArrived {
        id: "rid-fork".to_owned(),
        method: "eth_sendTransaction".to_owned(),
        params_json: params.to_string(),
        origin: "https://attack.example".to_owned(),
        transport_id: "tab-1".to_owned(),
        dedicated_transport: false,
        per_request_chain: Some(CHAIN),
        dapp: None,
        granted_address: Some(signer.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: 1_700_000_000_000.0,
        first_party: false,
    })
}

/// The shell's half of `ReadSafeOwner`, for real: the `eth_call` the core
/// built, to the fork, answered as the node answered it.
fn answer_reads(sut: &mut Sut, rpc_url: &str) -> usize {
    let mut answered = 0;
    loop {
        let read = sut.outstanding().into_iter().find_map(|op| match op {
            Op::ReadSafeOwner {
                id,
                chain_id,
                safe,
                data,
                ..
            } => Some((id, chain_id, safe, data)),
            _ => None,
        });
        let Some((id, chain_id, safe, data)) = read else {
            return answered;
        };
        let answer = match rpc_at(
            rpc_url,
            "eth_call",
            &json!([{ "to": safe, "data": data }, "latest"]),
        ) {
            Ok(envelope) => match (envelope.get("error"), envelope["result"].as_str()) {
                (Some(error), _) => SafeOwnerAnswer::RpcError {
                    code: error["code"].as_i64(),
                    message: error["message"].as_str().unwrap_or_default().to_owned(),
                },
                (None, Some(data)) => SafeOwnerAnswer::Returned {
                    data: data.to_owned(),
                },
                (None, None) => SafeOwnerAnswer::Unreachable {
                    detail: "no result".to_owned(),
                },
            },
            Err(detail) => SafeOwnerAnswer::Unreachable { detail },
        };
        let safe_match = safe.clone();
        sut.resolve_matching(
            |op| matches!(op, Op::ReadSafeOwner { safe: s, .. } if *s == safe_match),
            SignShellResult::SafeOwnerRead {
                id,
                chain_id,
                safe,
                answer,
            },
        );
        answered += 1;
    }
}

fn signs(ops: &[Op]) -> bool {
    ops.iter().any(|op| {
        matches!(
            op,
            Op::SignAndSubmit { .. } | Op::CheckBundlerFunding { .. }
        )
    })
}

/// Approve, then answer the pre-check: whether the request reaches the
/// passkey + submit pipeline.
fn approve_reaches_submit(sut: &mut Sut) -> bool {
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    if !ops
        .iter()
        .any(|op| matches!(op, Op::CheckBundlerFunding { .. }))
    {
        return false;
    }
    let ops = sut.resolve(SignShellResult::PreCheck { funding: None });
    sut.outstanding()
        .iter()
        .chain(ops.iter())
        .any(|op| matches!(op, Op::SignAndSubmit { .. }))
}

#[test]
#[ignore = "needs a Gnosis fork: VELA_PARENT_FORK=http://127.0.0.1:<port>"]
fn the_appendix_takeover_is_real_on_the_fork_and_vela_refuses_it() {
    let chain_id = rpc("eth_chainId", &json!([]));
    assert_eq!(chain_id, "0x64", "a Gnosis fork");
    // The whole run on a snapshot of its own: the fork is left as found.
    let outer = rpc("evm_snapshot", &json!([]));
    let salt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x0099, |t| t.as_secs());
    rpc("anvil_setBalance", &json!([OWNER, "0x56bc75e2d63100000"]));
    let safe = make_safe(OWNER, salt);
    let data = takeover_data(&safe, OWNER);
    println!("outside Safe {safe}, owned by {OWNER} alone");

    // 1. Without Vela: the owner's address sends the page's call — the Safe
    //    is the attacker's in one transaction. Snapshot, prove, revert.
    let snapshot = rpc("evm_snapshot", &json!([]));
    let receipt = send_from(OWNER, &safe, &data, "0x0");
    assert_eq!(receipt["status"], "0x1", "{receipt}");
    assert_eq!(
        owners(&safe),
        vec![ATTACKER.to_ascii_lowercase()],
        "taken over: the attacker is the only owner"
    );
    println!("direct: execTransaction(swapOwner) v=1 from the owner -> owners = [{ATTACKER}]");
    assert_eq!(rpc("evm_revert", &json!([snapshot])), json!(true));
    assert_eq!(owners(&safe), vec![OWNER.to_ascii_lowercase()], "reverted");

    // 2. Through Vela 0.9.9: the same request, from a site.
    let params = json!([{ "from": OWNER, "to": safe, "data": data, "value": "0x0" }]);
    let mut sut = boot(OWNER);
    let ops = arrive(&mut sut, OWNER, &params);
    assert!(!signs(&ops), "nothing is signed at arrival");
    assert_eq!(answer_reads(&mut sut, &fork()), 1, "one isOwner read");
    let view = sut.view();
    let blocked = view
        .blocked
        .clone()
        .expect("refused once the chain answered");
    assert_eq!(blocked.parent.as_deref(), Some(safe.as_str()));
    assert_eq!(blocked.reason, Some(ParentReason::Owner));
    assert_eq!(blocked.function, "execTransaction");
    assert!(!view.confirm_gate_open);
    assert!(!approve_reaches_submit(&mut sut), "never signed");
    let closed = sut.dispatch(Event::SwipeDismissed);
    assert!(
        closed.iter().any(|op| matches!(
            op,
            Op::SendResponse { payload: SignResponsePayload::Err { code, kind, .. }, .. }
                if *code == CODE_INTERNAL && *kind == SignErrorKind::ParentSafeBlocked
        )),
        "the site hears -32603 parent_safe_blocked: {closed:?}"
    );
    assert_eq!(owners(&safe), vec![OWNER.to_ascii_lowercase()], "untouched");
    println!("through Vela 0.9.9: refused (owner), -32603 parent_safe_blocked; owners unchanged");

    // 2b. The same, when the read cannot be made: refused "couldn't check".
    let mut sut = boot(OWNER);
    arrive(&mut sut, OWNER, &params);
    assert_eq!(answer_reads(&mut sut, "http://127.0.0.1:1"), 1);
    assert_eq!(
        sut.view().blocked.and_then(|b| b.reason),
        Some(ParentReason::Unchecked)
    );
    assert!(!approve_reaches_submit(&mut sut));

    // 3. A normal dApp transaction still goes through: a plain transfer, and
    //    the same execTransaction shape on a Safe this account does NOT own
    //    (the chain says `false`).
    let other = make_safe(ATTACKER, salt + 1);
    for (what, params) in [
        (
            "a plain transfer",
            json!([{ "from": OWNER, "to": "0x00000000000000000000000000000000000beef0", "value": "0x1" }]),
        ),
        (
            "execTransaction on a Safe it does not own",
            json!([{ "from": OWNER, "to": other, "data": takeover_data(&other, ATTACKER), "value": "0x0" }]),
        ),
    ] {
        let mut sut = boot(OWNER);
        arrive(&mut sut, OWNER, &params);
        answer_reads(&mut sut, &fork());
        assert!(sut.view().blocked.is_none(), "{what}: not refused");
        assert!(
            approve_reaches_submit(&mut sut),
            "{what}: reaches the passkey + submit"
        );
        println!("normal: {what} -> SignAndSubmit");
    }
    // …and on the chain, the plain transfer lands.
    let receipt = send_from(
        OWNER,
        "0x00000000000000000000000000000000000beef0",
        "0x",
        "0x1",
    );
    assert_eq!(receipt["status"], "0x1");
    println!("normal: the plain transfer lands on the fork");
    assert_eq!(rpc("evm_revert", &json!([outer])), json!(true));
}

// ---------------------------------------------------------------------------
// 0.9.10: the same takeover with the Safe's address written without its `0x`
// ---------------------------------------------------------------------------

/// The 4337 module, the EntryPoint it trusts, and the module-setup contract a
/// Vela account is made with (`vela_core::safe`).
const MODULE: &str = vela_core::safe::SAFE_4337_MODULE;
const ENTRY_POINT: &str = vela_core::safe::ENTRY_POINT;
const MODULE_SETUP: &str = vela_core::safe::SAFE_MODULE_SETUP;
/// The key that controls the account (irrelevant to the attack: the account
/// itself is the parent's owner).
const ACCOUNT_KEY: &str = "0x00000000000000000000000000000000000a11c0";

/// A Safe v1.4.1 shaped like a Vela account: the 4337 module enabled
/// (`SAFE_MODULE_SETUP.enableModules`) and set as the fallback handler — so
/// the EntryPoint runs its calls through `executeUserOp`, as on any Vela
/// account; its address.
fn make_vela_shaped_account(salt: u64) -> String {
    let enable = calldata(
        "enableModules(address[])",
        vec![DynSolValue::Array(vec![addr(MODULE)])],
    );
    let setup = calldata(
        "setup(address[],uint256,address,bytes,address,address,uint256,address)",
        vec![
            DynSolValue::Array(vec![addr(ACCOUNT_KEY)]),
            uint(1),
            addr(MODULE_SETUP),
            bytes(&enable),
            addr(MODULE),
            addr("0x0000000000000000000000000000000000000000"),
            uint(0),
            addr("0x0000000000000000000000000000000000000000"),
        ],
    );
    let create = calldata(
        "createProxyWithNonce(address,bytes,uint256)",
        vec![addr(SINGLETON), bytes(&setup), uint(salt)],
    );
    let predicted = call(FACTORY, &create);
    let account = format!(
        "0x{}",
        predicted.trim_start_matches("0x")[24..64].to_ascii_lowercase()
    );
    let receipt = send_from(OWNER, FACTORY, &create, "0x0");
    assert_eq!(receipt["status"], "0x1", "the account is made: {receipt}");
    let enabled = call(
        &account,
        &calldata("isModuleEnabled(address)", vec![addr(MODULE)]),
    );
    assert!(enabled.ends_with('1'), "the 4337 module is enabled");
    account
}

/// What Vela signs for a site's single call (`tx_request::calls_of` then
/// `user_op::build_execute_call_data`) — the user operation's `callData`,
/// which the EntryPoint hands to the account.
fn vela_call_data(params: &Value) -> String {
    let calls = vela_core::tx_request::calls_of("eth_sendTransaction", &params.to_string())
        .expect("the submit reads the call");
    assert_eq!(calls.len(), 1);
    let one = vela_core::user_op::to_multi_send_call(&calls[0].to, &calls[0].value, &calls[0].data)
        .unwrap();
    let data = vela_core::user_op::build_execute_call_data(&one.to, &one.value_hex, &one.data)
        .expect("the encoder takes the address");
    format!("0x{}", hex::encode(data))
}

/// The EntryPoint running `call_data` on `account`, as it does once a user
/// operation validates (the signature step is the passkey's; not the point).
fn run_as_entry_point(account: &str, call_data: &str) -> Value {
    rpc(
        "anvil_setBalance",
        &json!([ENTRY_POINT, "0x56bc75e2d63100000"]),
    );
    send_from(ENTRY_POINT, account, call_data, "0x0")
}

fn invalid_params_message(ops: &[Op]) -> Option<String> {
    ops.iter().find_map(|op| match op {
        Op::SendResponse {
            payload: SignResponsePayload::Err { code, message, .. },
            ..
        } if *code == CODE_INVALID_PARAMS => Some(message.clone().unwrap_or_default()),
        _ => None,
    })
}

fn arrive_method(sut: &mut Sut, signer: &str, method: &str, params: &Value) -> Vec<Op> {
    sut.dispatch(Event::RequestArrived {
        id: "rid-fork".to_owned(),
        method: method.to_owned(),
        params_json: params.to_string(),
        origin: "https://attack.example".to_owned(),
        transport_id: "tab-1".to_owned(),
        dedicated_transport: false,
        per_request_chain: Some(CHAIN),
        dapp: None,
        granted_address: Some(signer.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: 1_700_000_000_000.0,
        first_party: false,
    })
}

fn wei(value: &Value) -> U256 {
    U256::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

#[test]
#[ignore = "needs a Gnosis fork: VELA_PARENT_FORK=http://127.0.0.1:<port>"]
fn the_bare_hex_takeover_is_real_on_the_fork_and_vela_0_9_10_refuses_it() {
    assert_eq!(rpc("eth_chainId", &json!([])), "0x64", "a Gnosis fork");
    let outer = rpc("evm_snapshot", &json!([]));
    let salt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x0910, |t| t.as_secs())
        + 0x0910;
    rpc("anvil_setBalance", &json!([OWNER, "0x56bc75e2d63100000"]));
    let account = make_vela_shaped_account(salt);
    let parent = make_safe(&account, salt + 1);
    let data = takeover_data(&parent, &account);
    let bare = parent.trim_start_matches("0x").to_owned();
    println!("account {account} (4337 module), parent Safe {parent} owned by it alone");

    // 1. What 0.9.9 let through: the site writes the parent's address without
    //    `0x`. Vela's own encoder reads it as the parent — the same bytes as
    //    the 0x form — and the EntryPoint running it hands the parent to the
    //    attacker. Snapshot, prove, revert.
    let bare_params = json!([{ "from": account, "to": bare, "data": data, "value": "0x0" }]);
    let canonical_params = json!([{ "from": account, "to": parent, "data": data, "value": "0x0" }]);
    let signed = vela_call_data(&bare_params);
    assert_eq!(
        signed,
        vela_call_data(&canonical_params),
        "the encoder reads the bare form as the Safe"
    );
    let snapshot = rpc("evm_snapshot", &json!([]));
    let receipt = run_as_entry_point(&account, &signed);
    assert_eq!(receipt["status"], "0x1", "{receipt}");
    assert_eq!(
        owners(&parent),
        vec![ATTACKER.to_ascii_lowercase()],
        "taken over through the account's executeUserOp"
    );
    println!(
        "as signed: executeUserOp(<bare parent>, execTransaction(swapOwner) v=1) -> owners = [{ATTACKER}]"
    );
    assert_eq!(rpc("evm_revert", &json!([snapshot])), json!(true));
    assert_eq!(owners(&parent), vec![account.clone()], "reverted");

    // 2. Through Vela 0.9.10: refused at arrival, -32602, before any read,
    //    sheet or submit — the bare form, an upper-case `0X`, a batch leg.
    for (method, params) in [
        ("eth_sendTransaction", bare_params.clone()),
        (
            "eth_sendTransaction",
            json!([{ "from": account, "to": format!("0X{bare}"), "data": data, "value": "0x0" }]),
        ),
        (
            "wallet_sendCalls",
            json!([{ "version": "2.0.0", "from": account, "chainId": "0x64", "atomic": true,
                     "calls": [{ "to": bare, "data": data, "value": "0x0" }] }]),
        ),
    ] {
        let mut sut = boot(&account);
        let ops = arrive_method(&mut sut, &account, method, &params);
        let message = invalid_params_message(&ops).unwrap_or_else(|| panic!("{method}: {ops:?}"));
        assert!(
            message.contains("expected 0x followed by 40 hex characters"),
            "{message}"
        );
        assert!(!signs(&ops), "{method}: nothing signed");
        assert!(
            !ops.iter().any(|op| matches!(op, Op::ReadSafeOwner { .. })),
            "{method}: refused before any read"
        );
        assert_eq!(
            sut.view().surface,
            SignSurface::Hidden,
            "{method}: no sheet"
        );
        println!("through Vela 0.9.10: {method} with a non-0x `to` -> -32602 \"{message}\"");
    }
    assert_eq!(owners(&parent), vec![account.clone()], "untouched");

    // 2b. The 0x form is still the 0.9.9 guard's: read on the fork, owned,
    //     refused.
    let mut sut = boot(&account);
    arrive(&mut sut, &account, &canonical_params);
    assert_eq!(answer_reads(&mut sut, &fork()), 1, "one isOwner read");
    let blocked = sut.view().blocked.expect("refused");
    assert_eq!(blocked.parent.as_deref(), Some(parent.as_str()));
    assert_eq!(blocked.reason, Some(ParentReason::Owner));
    assert!(!approve_reaches_submit(&mut sut), "never signed");
    println!("through Vela 0.9.10: the 0x form -> refused (owner)");

    // 3. A normal dApp transaction still works: a plain transfer from the
    //    account reaches the passkey + submit, and what Vela signs for it
    //    lands when the EntryPoint runs it.
    rpc("anvil_setBalance", &json!([account, "0xde0b6b3a7640000"]));
    let payee = "0x00000000000000000000000000000000000beef1";
    let transfer = json!([{ "from": account, "to": payee, "value": "0x2386f26fc10000" }]);
    let mut sut = boot(&account);
    let ops = arrive(&mut sut, &account, &transfer);
    assert!(invalid_params_message(&ops).is_none(), "{ops:?}");
    answer_reads(&mut sut, &fork());
    assert!(sut.view().blocked.is_none(), "not refused");
    assert!(
        approve_reaches_submit(&mut sut),
        "reaches the passkey + submit"
    );
    let before = wei(&rpc("eth_getBalance", &json!([payee, "latest"])));
    let receipt = run_as_entry_point(&account, &vela_call_data(&transfer));
    assert_eq!(receipt["status"], "0x1", "{receipt}");
    let after = wei(&rpc("eth_getBalance", &json!([payee, "latest"])));
    assert_eq!(
        after - before,
        U256::from(10_000_000_000_000_000_u64),
        "0.01 xDAI arrived"
    );
    println!("normal: a plain transfer -> SignAndSubmit, and it lands on the fork (0.01 xDAI)");
    assert_eq!(rpc("evm_revert", &json!([outer])), json!(true));
}
