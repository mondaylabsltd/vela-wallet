//! §37 (review 1-11): ERC-6492, proved against a real verifier on the fork
//! harness (`scripts/onchain/multisig-fork/up.sh`, a Gnosis fork with Safe's
//! canonical contracts).
//!
//! This half builds every signature the way the wallet does — the fixture
//! key signs the Safe's `SafeMessage` hash of the request's own hash
//! (`sign_message::original_hash` → `user_op::compute_safe_message_hash`),
//! the core envelopes it (`eip1271_envelope_signature`) and answers it
//! (`message_signature`, with the account's code read from the fork) — for a
//! `personal_sign`, a typed-data request and a sign-in (EIP-4361), from an
//! account that is NOT deployed (the ERC-6492 envelope) and from one that is
//! (the bare EIP-1271 signature), and writes them out. The other half,
//! `scripts/onchain/erc6492/verify.mjs`, hands each to viem's
//! `verifyMessage` / `verifyTypedData` (universal ERC-6492 validation, the
//! deployless UniversalSigValidator) on the same fork — and checks that a
//! tampered one, and an undeployed account's BARE signature, do not verify.
//!
//! Ignored by default; run with the harness up:
//!
//! ```sh
//! VELA_MULTISIG_FORK=1 FORK_PORT=9745 VELA_6492_OUT=/tmp/harness-r22b-1/erc6492.json \
//!   cargo test -p vela-core --features crux,dev-fixtures --test erc6492_fork -- --ignored --nocapture
//! node scripts/onchain/erc6492/verify.mjs /tmp/harness-r22b-1/erc6492.json http://127.0.0.1:9745
//! ```

#![cfg(feature = "dev-fixtures")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use vela_core::dev_fixtures as fixtures;
use vela_core::primitives;
use vela_core::user_op::{self, WalletKey};

fn rpc(port: u16, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let host = format!("127.0.0.1:{port}");
    let mut s = TcpStream::connect(&host).expect("connects");
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let req = format!(
        "POST / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).expect("writes");
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).expect("reads");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (_, payload) = text.split_once("\r\n\r\n").expect("a head");
    let start = payload.find('{').unwrap_or(0);
    let answer: Value =
        serde_json::from_str(&payload[start..payload.rfind('}').map_or(payload.len(), |e| e + 1)])
            .unwrap_or_else(|e| panic!("not JSON ({e}): {payload}"));
    if let Some(error) = answer.get("error") {
        panic!("{method}: {error}");
    }
    answer["result"].clone()
}

/// A test-only P-256 scalar of this proof's own (no funds): its single-key
/// Safe is an address no other suite on the harness touches.
fn account(scalar: &str, name: &'static str, tag: &[u8]) -> fixtures::FixtureAccount {
    use p256::ecdsa::SigningKey;
    let signing = SigningKey::from_slice(&primitives::from_hex(scalar).unwrap()).unwrap();
    let public_key_hex = primitives::to_hex(
        signing.verifying_key().to_sec1_point(false).as_bytes(),
        false,
    );
    let key = vela_core::safe::parse_public_key(&public_key_hex).unwrap();
    let address = vela_core::safe::compute_safe_address(&key.x, &key.y)
        .unwrap()
        .address;
    fixtures::FixtureAccount {
        index: 98,
        credential_id_hex: primitives::to_hex(tag, false),
        name,
        // The struct wants a 'static: these two live as long as the test.
        private_key_hex: Box::leak(scalar.to_owned().into_boxed_str()),
        public_key_hex,
        address,
    }
}

const UNDEPLOYED_KEY: &str = "6c2f8e1a4b7d0c3e9f5a2b8d1e4c7f0a3b6d9e2c5f8a1b4d7e0c3f6a9b2d5e81";
const DEPLOYED_KEY: &str = "1d4a7c0f3e6b9d2a5c8f1e4b7d0a3c6f9e2b5d8a1c4f7e0b3d6a9c2f5e8b1d42";

const TYPED: &str = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"chainId","type":"uint256"}],"Mail":[{"name":"from","type":"string"},{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Ether Mail","version":"1","chainId":100},"message":{"from":"Vela","contents":"Hello, Bob!"}}"#;

fn keys_of(a: &fixtures::FixtureAccount) -> Vec<WalletKey> {
    vec![WalletKey {
        credential_id: a.credential_id_hex.clone(),
        public_key_hex: a.public_key_hex.clone(),
    }]
}

/// The site's answer to `method` with `params`, signed as the wallet signs it.
fn answer(
    port: u16,
    chain_id: u64,
    a: &fixtures::FixtureAccount,
    method: &str,
    params: &Value,
) -> (String, String) {
    let original = vela_core::sign_message::original_hash(method, &params.to_string()).unwrap();
    let challenge = user_op::compute_safe_message_hash(&original, chain_id, &a.address).unwrap();
    let assertion =
        fixtures::build_assertion(a, &challenge, fixtures::RP_ID, fixtures::ORIGIN).unwrap();
    let bare = user_op::eip1271_envelope_signature(
        &primitives::from_hex(&assertion.authenticator_data_hex).unwrap(),
        &primitives::from_hex(&assertion.client_data_json_hex).unwrap(),
        &primitives::from_hex(&assertion.signature_der_hex).unwrap(),
        &a.credential_id_hex,
        &keys_of(a),
    )
    .unwrap();
    let code = rpc(port, "eth_getCode", json!([a.address, "latest"]));
    let answered =
        user_op::message_signature(&bare, &keys_of(a), &a.address, code.as_str()).unwrap();
    (
        primitives::to_hex(&answered, true),
        primitives::to_hex(&bare, true),
    )
}

fn cases(port: u16, chain_id: u64, a: &fixtures::FixtureAccount, deployed: bool) -> Vec<Value> {
    let text = "Hello from Vela (§37 ERC-6492)";
    let siwe = format!(
        "dapp.example wants you to sign in with your Ethereum account:\n{}\n\nSign in.\n\nURI: https://dapp.example\nVersion: 1\nChain ID: {chain_id}\nNonce: 32891756\nIssued At: 2026-11-08T00:00:00Z",
        a.address
    );
    let mut out = Vec::new();
    for (kind, method, params, payload) in [
        (
            "personal_sign",
            "personal_sign",
            json!([text, a.address]),
            json!(text),
        ),
        (
            "siwe",
            "personal_sign",
            json!([siwe, a.address]),
            json!(siwe),
        ),
        (
            "typed_data",
            "eth_signTypedData_v4",
            json!([a.address, TYPED]),
            serde_json::from_str::<Value>(TYPED).unwrap(),
        ),
    ] {
        let (signature, bare) = answer(port, chain_id, a, method, &params);
        let wrapped = signature.ends_with(&"6492".repeat(16));
        assert_eq!(
            wrapped, !deployed,
            "{kind}: wrapped exactly when undeployed"
        );
        if deployed {
            assert_eq!(signature, bare);
        }
        out.push(json!({
            "kind": kind, "address": a.address, "deployed": deployed,
            "message": payload, "signature": signature, "bare": bare,
        }));
    }
    out
}

#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh)"]
fn erc6492_signatures_for_the_fork() {
    if std::env::var("VELA_MULTISIG_FORK").is_err() {
        return;
    }
    let port: u16 = std::env::var("FORK_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8545);
    let chain_id = u64::from_str_radix(
        rpc(port, "eth_chainId", json!([]))
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();

    let undeployed = account(UNDEPLOYED_KEY, "R22 undeployed", b"vela-r22-6492-a");
    assert_eq!(
        rpc(port, "eth_getCode", json!([undeployed.address, "latest"])),
        json!("0x"),
        "the undeployed account has no code on the fork"
    );
    let mut all = cases(port, chain_id, &undeployed, false);

    // The deployed one: its own createProxyWithNonce, sent from the fork's
    // first unlocked account — exactly what the 6492 envelope asks a verifier
    // to simulate.
    let deployed = account(DEPLOYED_KEY, "R22 deployed", b"vela-r22-6492-b");
    if rpc(port, "eth_getCode", json!([deployed.address, "latest"])) == json!("0x") {
        let init = user_op::build_init_code_for_keys(&[deployed.public_key_hex.clone()]).unwrap();
        let from = rpc(port, "eth_accounts", json!([]))[0].clone();
        let tx = rpc(
            port,
            "eth_sendTransaction",
            json!([{
                "from": from,
                "to": primitives::to_hex(&init[..20], true),
                "data": primitives::to_hex(&init[20..], true),
                "gas": "0x7a1200",
            }]),
        );
        assert!(tx.is_string(), "{tx}");
    }
    let code = rpc(port, "eth_getCode", json!([deployed.address, "latest"]));
    assert_ne!(code, json!("0x"), "deployed by its own factory call");
    all.extend(cases(port, chain_id, &deployed, true));

    let out = json!({ "chainId": chain_id, "cases": all });
    if let Ok(path) = std::env::var("VELA_6492_OUT") {
        std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
        println!("wrote {path}");
    }
}
