//! §35 (review 1-6): a SafeOp's validity window, proved against the real
//! contracts on the fork harness (`scripts/onchain/multisig-fork/up.sh`) — the
//! canonical EntryPoint v0.7 and Safe4337Module v0.3.0 on a Gnosis fork.
//!
//! What it proves, each from the chain's own answer:
//!
//! 1. the module hashes the window exactly as the core does —
//!    `Safe4337Module.getOperationHash` equals `calculate_safe_op_hash` for
//!    windows in both orders of magnitude (so `validAfter` and `validUntil` are
//!    not swapped anywhere: the Certora note on Safe's README example);
//! 2. an operation submitted past its `validUntil` is refused by the EntryPoint
//!    (`AA22 expired or not due`), and the core reads that refusal as
//!    `RelayRejection::Expired` — nothing was sent;
//! 3. an operation submitted before its `validAfter` is refused the same way,
//!    and the same operation lands once the chain's clock passes `validAfter`
//!    (the order of the two 6-byte fields, proved by the EntryPoint);
//! 4. the wallet's own window (`stamp_validity`: 0 … now + 30 min) lands —
//!    straight into `handleOps`, and through the harness relay's
//!    `eth_sendUserOperation` (the 12 bytes reach the chain untouched).
//!
//! Every operation is a fixture account's first (it deploys the Safe through
//! `initCode`), signed by the fixture key over the core's hash and enveloped
//! by the core; nothing here hand-builds a signature. Each case runs on an
//! anvil snapshot and is reverted, so the fork is left as it was found.
//!
//! Ignored by default; run with the harness up:
//!
//! ```sh
//! VELA_MULTISIG_FORK=1 FORK_PORT=9745 RELAY_PORT=9790 \
//!   cargo test -p vela-core --features crux,dev-fixtures --test safe_op_window_fork -- --ignored --nocapture
//! ```

#![cfg(feature = "dev-fixtures")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use alloy_dyn_abi::DynSolValue;
use alloy_primitives::{Address, B256, U256};
use serde_json::{json, Value};
use vela_core::dev_fixtures as fixtures;
use vela_core::primitives::{self, function_selector};
use vela_core::safe::{ENTRY_POINT, SAFE_4337_MODULE};
use vela_core::user_op::{
    self, build_init_code_for_keys, build_multi_send_execute_call_data, MultiSendCall,
    UserOperation, WalletKey,
};

fn port(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(default)
}

fn post(url: &str, body: &str) -> Value {
    let rest = url.strip_prefix("http://").expect("http");
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let mut s = TcpStream::connect(host).expect("connects");
    s.set_read_timeout(Some(Duration::from_secs(120))).ok();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).expect("writes");
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).expect("reads");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (_, payload) = text.split_once("\r\n\r\n").expect("a head");
    // The harness relay answers chunked from Python's http.server only when
    // asked; plain JSON either way.
    let start = payload.find('{').unwrap_or(0);
    serde_json::from_str(&payload[start..payload.rfind('}').map_or(payload.len(), |e| e + 1)])
        .unwrap_or_else(|e| panic!("not JSON ({e}): {payload}"))
}

struct Chain {
    rpc: String,
    relay: String,
}

impl Chain {
    fn call(&self, method: &str, params: Value) -> Result<Value, (String, Option<Value>)> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let answer = post(&self.rpc, &body.to_string());
        match answer.get("error") {
            Some(e) => Err((
                e["message"].as_str().unwrap_or("").to_owned(),
                e.get("data").cloned(),
            )),
            None => Ok(answer["result"].clone()),
        }
    }

    fn ok(&self, method: &str, params: Value) -> Value {
        self.call(method, params)
            .unwrap_or_else(|e| panic!("{method}: {e:?}"))
    }

    fn head_time(&self) -> u64 {
        let block = self.ok("eth_getBlockByNumber", json!(["latest", false]));
        u64::from_str_radix(
            block["timestamp"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap()
    }

    fn snapshot(&self) -> Value {
        self.ok("evm_snapshot", json!([]))
    }

    fn revert(&self, id: &Value) {
        assert_eq!(self.ok("evm_revert", json!([id])), json!(true));
    }
}

const BUNDLER: &str = "0x6a3a1D2F2b1b2C7c9A1b6dF0e5f4d0c3B2a19876";

/// A test-only key of this proof's own (a fixed P-256 scalar, no funds), so
/// its single-key Safe is an address no other suite on the harness touches.
/// (The parallel space's three fixtures are the multisig suites' members:
/// `multisig_fork` sends from their addresses by impersonation, which gives
/// them a nonce, and a CREATE2 deployment onto an address with a nonce fails
/// AA13.) It signs through the fixture signer, as the parallel space does.
const WINDOW_KEY: &str = "4b5f1a3e8d2c6b9f0e7a1d4c8b3f6e2a9d5c1b7e4f0a3d6c9b2e8f1a5d4c7b30";

fn fixture() -> fixtures::FixtureAccount {
    use p256::ecdsa::SigningKey;
    let signing = SigningKey::from_slice(&primitives::from_hex(WINDOW_KEY).unwrap()).unwrap();
    let public_key_hex = primitives::to_hex(
        signing.verifying_key().to_sec1_point(false).as_bytes(),
        false,
    );
    let key = vela_core::safe::parse_public_key(&public_key_hex).unwrap();
    let address = vela_core::safe::compute_safe_address(&key.x, &key.y)
        .unwrap()
        .address;
    fixtures::FixtureAccount {
        index: 99,
        credential_id_hex: primitives::to_hex(b"vela-r21-window", false),
        name: "R21 window",
        private_key_hex: WINDOW_KEY,
        public_key_hex,
        address,
    }
}

/// The fixture account's first operation: deploy through `initCode`, one
/// zero-value call — maxFeePerGas 0, so the EntryPoint asks no prefund.
fn first_operation(chain: &Chain, account: &fixtures::FixtureAccount) -> UserOperation {
    let code = chain.ok("eth_getCode", json!([account.address, "latest"]));
    assert_eq!(code, json!("0x"), "the fixture is undeployed on the fork");
    UserOperation {
        sender: account.address.clone(),
        nonce: "0x0".to_owned(),
        init_code: build_init_code_for_keys(std::slice::from_ref(&account.public_key_hex)).unwrap(),
        call_data: build_multi_send_execute_call_data(&[MultiSendCall {
            to: "0x0000000000000000000000000000000000000004".to_owned(),
            value_hex: "0x0".to_owned(),
            data: Vec::new(),
        }])
        .unwrap(),
        verification_gas_limit: 2_000_000,
        call_gas_limit: 300_000,
        pre_verification_gas: 100_000,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        paymaster_and_data: Vec::new(),
        valid_after: 0,
        valid_until: 0,
        signature: Vec::new(),
    }
}

/// The fixture key signs the core's hash; the core envelopes it.
fn sign(op: &mut UserOperation, account: &fixtures::FixtureAccount) {
    let digest = user_op::signable_safe_op_hash(op, 100).unwrap();
    let signed =
        fixtures::build_assertion(account, &digest, fixtures::RP_ID, fixtures::ORIGIN).unwrap();
    let keys = [WalletKey {
        credential_id: account.credential_id_hex.clone(),
        public_key_hex: account.public_key_hex.clone(),
    }];
    op.signature = user_op::envelope_signature(
        op,
        &primitives::from_hex(&signed.authenticator_data_hex).unwrap(),
        &primitives::from_hex(&signed.client_data_json_hex).unwrap(),
        &primitives::from_hex(&signed.signature_der_hex).unwrap(),
        &signed.credential_id_hex,
        &keys,
    )
    .unwrap();
    assert_eq!(
        user_op::signature_window(&op.signature),
        Some((op.valid_after, op.valid_until))
    );
}

fn gas_word(high: u128, low: u128) -> DynSolValue {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&high.to_be_bytes());
    bytes[16..].copy_from_slice(&low.to_be_bytes());
    DynSolValue::FixedBytes(B256::from(bytes), 32)
}

/// `PackedUserOperation`, as the EntryPoint and the module take it.
fn packed(op: &UserOperation) -> DynSolValue {
    DynSolValue::Tuple(vec![
        DynSolValue::Address(op.sender.parse::<Address>().unwrap()),
        DynSolValue::Uint(
            U256::from(u64::from_str_radix(op.nonce.trim_start_matches("0x"), 16).unwrap()),
            256,
        ),
        DynSolValue::Bytes(op.init_code.clone()),
        DynSolValue::Bytes(op.call_data.clone()),
        gas_word(op.verification_gas_limit, op.call_gas_limit),
        DynSolValue::Uint(U256::from(op.pre_verification_gas), 256),
        gas_word(op.max_priority_fee_per_gas, op.max_fee_per_gas),
        DynSolValue::Bytes(op.paymaster_and_data.clone()),
        DynSolValue::Bytes(op.signature.clone()),
    ])
}

const PACKED: &str = "(address,uint256,bytes,bytes,bytes32,uint256,bytes32,bytes,bytes)";

fn handle_ops_calldata(op: &UserOperation) -> String {
    let mut data = function_selector(&format!("handleOps({PACKED}[],address)")).unwrap();
    data.extend(
        DynSolValue::Tuple(vec![
            DynSolValue::Array(vec![packed(op)]),
            DynSolValue::Address(BUNDLER.parse::<Address>().unwrap()),
        ])
        .abi_encode_params(),
    );
    primitives::to_hex(&data, true)
}

/// `FailedOp(uint256 opIndex, string reason)`'s reason, out of revert data.
fn failed_op_reason(data: &Value) -> String {
    let hex = data
        .as_str()
        .or_else(|| data.get("data").and_then(Value::as_str))
        .unwrap_or_else(|| panic!("no revert data: {data}"));
    let bytes = primitives::from_hex(hex).unwrap();
    assert_eq!(
        primitives::to_hex(&bytes[..4], true),
        primitives::to_hex(
            &function_selector("FailedOp(uint256,string)").unwrap(),
            true
        ),
        "the EntryPoint's FailedOp"
    );
    let decoded = alloy_dyn_abi::DynSolType::Tuple(vec![
        alloy_dyn_abi::DynSolType::Uint(256),
        alloy_dyn_abi::DynSolType::String,
    ])
    .abi_decode_params(&bytes[4..])
    .unwrap();
    let DynSolValue::Tuple(fields) = decoded else {
        unreachable!()
    };
    fields[1].as_str().unwrap().to_owned()
}

/// `handleOps` from the bundler, mined in one block at `time`: the receipt's
/// status, and — when it reverted — the EntryPoint's reason (read by
/// replaying the call at the same timestamp before it is mined).
fn submit_at(chain: &Chain, op: &UserOperation, time: u64) -> (bool, Option<String>) {
    let data = handle_ops_calldata(op);
    let tx = json!({"from": BUNDLER, "to": ENTRY_POINT, "data": data, "gas": "0xe4e1c0"});
    // The reason, at the very timestamp the transaction will be mined at
    // (block overrides: `eth_call`'s fourth parameter).
    let reason = match chain.call(
        "eth_call",
        json!([tx, "latest", {}, {"time": format!("0x{time:x}")}]),
    ) {
        Ok(_) => None,
        Err((message, data)) => Some(
            data.as_ref()
                .map_or_else(|| message.clone(), failed_op_reason),
        ),
    };
    chain.ok("anvil_impersonateAccount", json!([BUNDLER]));
    chain.ok("anvil_setBalance", json!([BUNDLER, "0x56bc75e2d63100000"]));
    chain.ok("evm_setNextBlockTimestamp", json!([time]));
    let hash = chain.ok("eth_sendTransaction", json!([tx]));
    let mut receipt = Value::Null;
    for _ in 0..120 {
        receipt = chain.ok("eth_getTransactionReceipt", json!([hash]));
        if !receipt.is_null() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let mined = u64::from_str_radix(
        chain.ok("eth_getBlockByHash", json!([receipt["blockHash"], false]))["timestamp"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    assert_eq!(mined, time, "mined at the timestamp asked");
    (receipt["status"] == json!("0x1"), reason)
}

fn landed(chain: &Chain, op: &UserOperation) -> bool {
    let code = chain.ok("eth_getCode", json!([op.sender, "latest"]));
    code != json!("0x")
}

fn chain() -> Option<Chain> {
    if std::env::var("VELA_MULTISIG_FORK").ok().as_deref() != Some("1") {
        eprintln!("VELA_MULTISIG_FORK=1 not set; skipped");
        return None;
    }
    Some(Chain {
        rpc: format!("http://127.0.0.1:{}", port("FORK_PORT", 8545)),
        relay: format!("http://127.0.0.1:{}/100", port("RELAY_PORT", 8790)),
    })
}

/// (1) The module's own digest is the core's, for any window.
#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn the_module_hashes_the_window_exactly_as_the_core_does() {
    let Some(chain) = chain() else { return };
    let account = fixture();
    let mut op = first_operation(&chain, &account);
    for (after, until) in [
        (0u64, 0u64),
        (0, 1_800_001_800),
        (1_700_000_000, 1_800_000_000),
        (0x0102_0304_0506, 0x0a0b_0c0d_0e0f),
    ] {
        op.valid_after = after;
        op.valid_until = until;
        // The module reads the window out of the signature's first 12 bytes;
        // the rest of the signature is not hashed.
        op.signature = user_op::build_user_op_signature(
            after,
            until,
            &[0u8; 37],
            "",
            &[1u8; 32],
            &[1u8; 32],
            vela_core::safe::WEBAUTHN_SIGNER,
        )
        .unwrap();
        let mut data = function_selector(&format!("getOperationHash({PACKED})")).unwrap();
        data.extend(DynSolValue::Tuple(vec![packed(&op)]).abi_encode_params());
        let on_chain = chain.ok(
            "eth_call",
            json!([{"to": SAFE_4337_MODULE, "data": primitives::to_hex(&data, true)}, "latest"]),
        );
        let core = primitives::to_hex(&user_op::calculate_safe_op_hash(&op, 100).unwrap(), true);
        assert_eq!(
            on_chain.as_str().unwrap().to_lowercase(),
            core.to_lowercase(),
            "window {after}..{until}"
        );
        eprintln!("window {after}..{until}: module digest = core digest {core}");
    }
}

/// (2) Past `validUntil`: refused `AA22`, read by the core as expired, the
/// Safe never deployed. (4) Inside it: lands.
#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn an_operation_past_valid_until_is_refused_and_one_inside_its_window_lands() {
    let Some(chain) = chain() else { return };
    let account = fixture();
    let now = chain.head_time() + 1;
    let mut op = first_operation(&chain, &account);
    // The wallet's own window, stamped at the chain's "now".
    user_op::stamp_validity(&mut op, (now * 1000) as f64);
    assert_eq!(
        (op.valid_after, op.valid_until),
        (0, now + user_op::SAFE_OP_LIFETIME_SECS)
    );
    sign(&mut op, &account);

    // One second past validUntil.
    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, op.valid_until + 1);
    let reason = reason.expect("a refusal");
    eprintln!(
        "past validUntil ({}): status ok={ok}, EntryPoint: {reason}",
        op.valid_until + 1
    );
    assert!(!ok, "handleOps reverted");
    assert!(reason.contains("AA22"), "{reason}");
    assert!(
        !landed(&chain, &op),
        "nothing was sent: the Safe is not deployed"
    );
    // The relay would hand the wallet this reason; the core reads it.
    assert_eq!(
        user_op::classify_relay_rejection(&reason),
        user_op::RelayRejection::Expired
    );
    let verdict = user_op::submit_step(
        &user_op::SubmitReply::Error(json!({"code": -32500, "message": reason}).to_string()),
        0,
        false,
        &user_op::user_op_hash(&op, 100).unwrap(),
    );
    assert_eq!(
        verdict,
        user_op::SubmitStep::Done(user_op::SubmitVerdict::NotSent {
            rejection: Some(user_op::RelayRejection::Expired)
        })
    );
    chain.revert(&snap);

    // At validUntil exactly: still valid (expired only past it).
    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, op.valid_until);
    eprintln!(
        "at validUntil ({}): status ok={ok}, reason {reason:?}",
        op.valid_until
    );
    assert!(ok && reason.is_none(), "{reason:?}");
    assert!(landed(&chain, &op));
    chain.revert(&snap);

    // Inside the window, a minute after it was stamped: lands.
    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, now + 60);
    eprintln!(
        "inside the window ({}): status ok={ok}, reason {reason:?}",
        now + 60
    );
    assert!(ok && reason.is_none(), "{reason:?}");
    assert!(landed(&chain, &op), "the Safe deployed and ran the call");
    chain.revert(&snap);
}

/// (3) Before `validAfter`: refused; the same operation lands once the clock
/// passes it — the 6-byte fields are in the order the EntryPoint reads.
#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn an_operation_before_valid_after_is_refused_and_lands_once_it_is_due() {
    let Some(chain) = chain() else { return };
    let account = fixture();
    let now = chain.head_time() + 1;
    let mut op = first_operation(&chain, &account);
    // Not the wallet's window (it signs validAfter 0): a window that opens in
    // an hour and closes in two, to make the order of the fields observable.
    op.valid_after = now + 3600;
    op.valid_until = now + 7200;
    sign(&mut op, &account);

    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, now + 60);
    let reason = reason.expect("a refusal");
    eprintln!(
        "before validAfter ({}): status ok={ok}, EntryPoint: {reason}",
        now + 60
    );
    assert!(!ok);
    assert!(reason.contains("AA22"), "{reason}");
    assert!(!landed(&chain, &op));
    chain.revert(&snap);

    // Swapped, the fields would read "opens in two hours, closes in one" —
    // never valid. In the order the core writes them, it is due past
    // validAfter.
    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, op.valid_after + 1);
    eprintln!(
        "past validAfter ({}): status ok={ok}, reason {reason:?}",
        op.valid_after + 1
    );
    assert!(ok && reason.is_none(), "{reason:?}");
    assert!(landed(&chain, &op));
    chain.revert(&snap);

    // And past its validUntil it is refused again.
    let snap = chain.snapshot();
    let (ok, reason) = submit_at(&chain, &op, op.valid_until + 1);
    eprintln!(
        "past validUntil ({}): status ok={ok}, reason {reason:?}",
        op.valid_until + 1
    );
    assert!(!ok && reason.is_some_and(|r| r.contains("AA22")));
    chain.revert(&snap);
}

/// (4) The wallet's window through the relay: `user_op_to_json` →
/// `eth_sendUserOperation` → the relay's `handleOps` — the 12 bytes arrive as
/// signed and the operation lands.
#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn the_relay_passes_the_window_through_and_the_operation_lands() {
    let Some(chain) = chain() else { return };
    let account = fixture();
    let snap = chain.snapshot();
    let now = chain.head_time() + 1;
    let mut op = first_operation(&chain, &account);
    // A call unique to this run: the harness relay remembers every hash it
    // was sent, and an earlier run's was reverted with its snapshot.
    op.call_data = build_multi_send_execute_call_data(&[MultiSendCall {
        to: "0x0000000000000000000000000000000000000004".to_owned(),
        value_hex: "0x0".to_owned(),
        data: now.to_be_bytes().to_vec(),
    }])
    .unwrap();
    user_op::stamp_validity(&mut op, (now * 1000) as f64);
    sign(&mut op, &account);
    let wire = user_op::user_op_to_json(&op, &[]);
    assert_eq!(
        wire["signature"].as_str().unwrap()[2..26],
        format!("{:012x}{:012x}", op.valid_after, op.valid_until)
    );
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "eth_sendUserOperation",
        "params": [wire, ENTRY_POINT]});
    let answer = post(&chain.relay, &body.to_string());
    let hash = answer["result"]
        .as_str()
        .unwrap_or_else(|| panic!("{answer}"));
    assert_eq!(
        hash.to_lowercase(),
        user_op::user_op_hash(&op, 100).unwrap()
    );
    let mut receipt = Value::Null;
    for _ in 0..60 {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "eth_getUserOperationReceipt", "params": [hash]});
        receipt = post(&chain.relay, &body.to_string())["result"].clone();
        if !receipt.is_null() {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    eprintln!("through the relay: {hash} success={}", receipt["success"]);
    assert_eq!(receipt["success"], json!(true), "{receipt}");
    assert!(landed(&chain, &op));
    chain.revert(&snap);
}
