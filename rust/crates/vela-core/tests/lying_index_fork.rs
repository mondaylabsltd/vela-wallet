//! §37 (review 1-7): a lying key index, against the real registry on the
//! fork harness's Gnosis fork (`scripts/onchain/multisig-fork/up.sh`). Every
//! `eth_call` the core asks is answered by the fork — the passkey registry
//! there holds Gnosis's real records — and every index answer is the lie.
//! Ethereum is not on the harness: its calls are answered as failed, as an
//! unreachable network is.
//!
//! 1. The golden key's real group (Gnosis unit 10), with one member's key
//!    swapped for an attacker's: the chain proves it wrong — the index is
//!    discarded and the chain's own members are used.
//! 2. A group the chain does not hold at all — the person's real key beside
//!    an attacker's, for a key the registry never saw — is "nobody could say"
//!    (`verified_by: none`): a sign-in saves it UNCONFIRMED (no receive
//!    address), and the confirm step, chain only, finds no group: still
//!    unconfirmed, never confirmed.
//! 3. The golden key's lie (its real key beside an attacker's): the confirm
//!    step reads every group the chain holds for the key, none makes that
//!    address — refuted.
//!
//! Ignored by default; run with the harness up:
//!
//! ```sh
//! VELA_MULTISIG_FORK=1 FORK_PORT=9745 \
//!   cargo test -p vela-core --test lying_index_fork -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use vela_core::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};
use vela_core::registry_resolve::{
    confirm_step, identity_of, unit_step, ConfirmStep, Identity, ResolveStep, VerifiedBy,
};

fn eth_call(port: u16, to: &str, data: &str) -> Option<String> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "eth_call",
        "params": [{"to": to, "data": data}, "latest"]})
    .to_string();
    let host = format!("127.0.0.1:{port}");
    let mut s = TcpStream::connect(&host).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let req = format!(
        "POST / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (_, payload) = text.split_once("\r\n\r\n")?;
    let start = payload.find('{')?;
    let v: Value = serde_json::from_str(&payload[start..=payload.rfind('}')?]).ok()?;
    v["result"].as_str().map(str::to_owned)
}

/// Answer a walk's requests: the index with `index`, Gnosis from the fork.
fn answer(
    port: u16,
    requests: Vec<LookupRequest>,
    index: Option<&str>,
    out: &mut Vec<LookupAnswer>,
) {
    for request in requests {
        let (id, body) = match request {
            LookupRequest::IndexGet { id, .. } => (id, index.map(str::to_owned)),
            LookupRequest::EthCall {
                id,
                chain_id,
                to,
                data,
            } => {
                let body = (chain_id == 100)
                    .then(|| eth_call(port, &to, &data))
                    .flatten();
                (id, body)
            }
        };
        out.push(LookupAnswer {
            id,
            outcome: if body.is_some() {
                LookupOutcome::Ok
            } else {
                LookupOutcome::Failed
            },
            body,
        });
    }
}

fn unit(port: u16, unit_id: u64, index: &str) -> (Option<Value>, String, VerifiedBy, bool) {
    let mut answers = Vec::new();
    for _ in 0..32 {
        match unit_step(unit_id, "index", &answers) {
            ResolveStep::Ask { requests } => answer(port, requests, Some(index), &mut answers),
            ResolveStep::Done {
                body,
                source,
                verified_by,
                index_discarded,
            } => {
                return (
                    body.and_then(|b| serde_json::from_str(&b).ok()),
                    source,
                    verified_by,
                    index_discarded,
                )
            }
        }
    }
    panic!("the unit walk never finished");
}

fn confirm(port: u16, key: &str, address: &str) -> (Identity, VerifiedBy) {
    let mut answers = Vec::new();
    for _ in 0..32 {
        match confirm_step(key, address, &answers) {
            ConfirmStep::Ask { requests } => answer(port, requests, None, &mut answers),
            ConfirmStep::Done {
                identity,
                verified_by,
            } => return (identity, verified_by),
        }
    }
    panic!("the confirm walk never finished");
}

fn address_of(keys: &[&str]) -> String {
    let parsed: Vec<_> = keys
        .iter()
        .map(|k| vela_core::safe::parse_public_key(k).unwrap())
        .collect();
    vela_core::safe::compute_safe_address_multi(&parsed)
        .unwrap()
        .address
}

const VECTOR: &str = include_str!("vectors/rules/lying-index.json");
const RECORDED: &str = include_str!(
    "../../../../app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json"
);

#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn a_lying_index_is_caught_or_left_unconfirmed_on_the_fork() {
    if std::env::var("VELA_MULTISIG_FORK").is_err() {
        eprintln!("VELA_MULTISIG_FORK not set: skipped");
        return;
    }
    let port: u16 = std::env::var("FORK_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8545);
    let vector: Value = serde_json::from_str(VECTOR).unwrap();
    let golden = vector["publicKey"].as_str().unwrap().to_owned();
    let attacker = vector["attacker"].as_str().unwrap().to_owned();
    let honest = vector["cases"][0]["index"].clone();
    let _ = RECORDED;

    // 1. The real group with one member forged: the fork proves it wrong.
    let mut forged = honest.clone();
    forged["members"]["items"][1]["publicKey"] = Value::from(attacker.clone());
    let (body, source, verified_by, discarded) = unit(port, 10, &forged.to_string());
    eprintln!("forged member: source {source}, verified_by {verified_by:?}, discarded {discarded}");
    assert!(discarded, "the index was believed");
    assert_eq!(
        (source.as_str(), verified_by),
        ("chain:100", VerifiedBy::Gnosis)
    );
    assert_eq!(
        body.as_ref().map(|b| b["members"].clone()),
        Some(honest["members"].clone())
    );

    // 2. A group the chain never held — a key the registry never saw, beside
    // an attacker's. Nobody can say: saved unconfirmed, never confirmed.
    let fresh = "04".to_owned()
        + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";
    let mut lie = honest.clone();
    lie["members"]["items"][0]["publicKey"] = Value::from(fresh.clone());
    lie["members"]["items"][1]["publicKey"] = Value::from(attacker.clone());
    lie["unit"]["groupPublicKey"] = Value::from(format!("0x{}", "11".repeat(33)));
    let (_, _, verified_by, discarded) = unit(port, 10, &lie.to_string());
    eprintln!("unknown group: verified_by {verified_by:?}, discarded {discarded}");
    assert_eq!(verified_by, VerifiedBy::None);
    assert_eq!(identity_of(3, verified_by), Identity::Unconfirmed);
    let lie_address = address_of(&[&fresh, &attacker]);
    assert_eq!(
        confirm(port, &fresh, &lie_address),
        (Identity::Unconfirmed, VerifiedBy::None),
        "no group on the chain: still unconfirmed, never confirmed"
    );

    // 3. The golden key's lie: refuted by the groups the chain holds for it.
    let golden_lie = address_of(&[&golden, &attacker]);
    let (identity, by) = confirm(port, &golden, &golden_lie);
    eprintln!("golden key's lie: {identity:?} by {by:?}");
    assert_eq!(identity, Identity::Refuted);

    // …and the golden key's real group is confirmed by the fork.
    let honest_keys: Vec<String> = honest["members"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["publicKey"].as_str().unwrap().to_owned())
        .collect();
    let refs: Vec<&str> = honest_keys.iter().map(String::as_str).collect();
    assert_eq!(
        confirm(port, &golden, &address_of(&refs)),
        (Identity::Confirmed, VerifiedBy::Gnosis)
    );
}
