//! R22 (§36): the lying-RPC proof on the fork harness. A node that rewrites
//! what a drain does to "No asset changes" cannot give the signing sheet a
//! plain verdict: the real pool machine asks it and a second, independent
//! node (each checked for its chain id first), the core compares what their
//! two answers mean, and the confirm is "… anyway" (held) whichever of the
//! two is asked first. Two honest nodes give the plain verdict — the test is
//! not refusing everything.
//!
//! The drain, simulated from a funded fixture account on the Gnosis fork:
//! wrap 1 xDAI into WXDAI, then send it to an attacker. Every node sees the
//! WXDAI `Transfer` out; the liar (`fork_proxy`) drops every log.
//!
//! Ignored by default; run with the harness up:
//!
//! ```sh
//! FORK_PORT=97xx … scripts/onchain/multisig-fork/up.sh
//! VELA_MULTISIG_FORK=1 FORK_PORT=97xx \
//!   cargo test -p vela-core --features crux,dev-fixtures --test signing_truth_fork -- --ignored --nocapture
//! ```

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod fork_proxy;
mod support;

use std::collections::BTreeMap;

use fork_proxy::{Kind, Proxy};
use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::corroborate::corroborated_envelope;
use vela_core::app::rpc_pool::{
    Event, RpcCallVerdict, RpcEndpointSeed, RpcKind, RpcOperation as Op, RpcPool,
    RpcShellResult as Res, RpcSource, RpcTransportOutcome,
};
use vela_core::app::sim_outcome::{verdict_of_envelope, SimVerdict};

const CHAIN: u32 = 100;
/// Gnosis's WXDAI (WETH9: `deposit()` payable, `transfer`).
const WXDAI: &str = "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d";
const ATTACKER: &str = "0xbAd00000000000000000000000000000000000bA";

fn fork_url() -> String {
    let port = std::env::var("FORK_PORT").unwrap_or_else(|_| "8545".into());
    format!("http://127.0.0.1:{port}")
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}

/// The drain's `eth_simulateV1` params, from `from`.
fn drain_params(from: &str) -> Value {
    let transfer = format!(
        "0xa9059cbb{:0>64}{:064x}",
        ATTACKER.trim_start_matches("0x").to_lowercase(),
        10u128.pow(18)
    );
    json!([{
        "blockStateCalls": [{
            "stateOverrides": { from: { "balance": "0x3635C9ADC5DEA00000" } },
            "calls": [
                { "from": from, "to": WXDAI, "value": "0xde0b6b3a7640000", "data": "0xd0e30db0" },
                { "from": from, "to": WXDAI, "value": "0x0", "data": transfer },
            ]
        }],
        "traceTransfers": true,
        "validation": false
    }, "latest"])
}

/// One corroborated `eth_simulateV1` through the REAL pool machine over
/// `nodes` (in tier order: the first is asked first), every POST and chain-id
/// probe made for real. The envelope the shell hands the core.
fn corroborated_simulation(nodes: &[(&str, RpcSource)], params: &Value) -> Value {
    let mut pool = DomainDriver::<RpcPool>::new();
    let mut bodies: BTreeMap<String, Value> = BTreeMap::new();
    pool.dispatch(Event::CallRequested {
        call_id: "sim".into(),
        chain_id: CHAIN,
        kind: RpcKind::Rpc,
        method: "eth_simulateV1".into(),
        now_ms: now(),
        person: false,
        corroborate: true,
    });
    let mut verdict = None;
    for _ in 0..64 {
        let Some(op) = pool.outstanding().first().cloned() else {
            break;
        };
        let res = match &op {
            Op::LoadPoolConfig { chain_id } => Res::PoolConfig {
                chain_id: *chain_id,
                rpc_endpoints: nodes
                    .iter()
                    .map(|(url, source)| RpcEndpointSeed {
                        url: (*url).to_owned(),
                        source: *source,
                    })
                    .collect(),
                bundler_endpoints: vec![],
                own_only: false,
                now_ms: now(),
            },
            Op::ProbeChainId { chain_id, url, .. } => {
                let body =
                    json!({"jsonrpc": "2.0", "id": 1, "method": "eth_chainId", "params": []});
                let reported = fork_proxy::post(url, &body.to_string())
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v["result"].as_str().map(str::to_owned))
                    .and_then(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).ok());
                Res::ChainIdProbed {
                    chain_id: *chain_id,
                    url: url.clone(),
                    reported,
                    latency_ms: 5.0,
                    now_ms: now(),
                }
            }
            Op::JsonRpcPost {
                call_id,
                url,
                method,
                ..
            } => {
                let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
                let outcome = match fork_proxy::post(url, &body.to_string())
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                {
                    Some(answer) => {
                        let error = answer.get("error").filter(|e| !e.is_null()).map(|e| {
                            vela_core::app::rpc_pool::RpcErrorInfo {
                                code: e["code"].as_i64().and_then(|c| i32::try_from(c).ok()),
                                message: e["message"].as_str().map(str::to_owned),
                            }
                        });
                        bodies.insert(url.clone(), answer);
                        RpcTransportOutcome::Response { error }
                    }
                    None => RpcTransportOutcome::NotConnected,
                };
                Res::PostOutcome {
                    call_id: call_id.clone(),
                    url: url.clone(),
                    outcome,
                    latency_ms: 20.0,
                    now_ms: now(),
                }
            }
            Op::Conclude { verdict: v, .. } => {
                verdict = Some(v.clone());
                Res::Concluded
            }
            Op::PersistBans { .. } => Res::Persisted,
            Op::DrawJitter { call_id } => Res::Jitter {
                call_id: call_id.clone(),
                value: 0.0,
            },
            Op::StartBackoff { call_id, .. } => Res::BackoffElapsed {
                call_id: call_id.clone(),
                now_ms: now(),
            },
        };
        pool.resolve_matching(|o| *o == op, res);
    }
    match verdict.expect("the pool concluded") {
        RpcCallVerdict::Respond {
            url,
            witness_url,
            own_node,
            sole_provider,
            ..
        } => {
            eprintln!("  first {url} · witness {witness_url:?} · own node {own_node}");
            corroborated_envelope(
                bodies.get(&url),
                witness_url.as_ref().and_then(|w| bodies.get(w)),
                own_node,
                sole_provider,
            )
        }
        other => panic!("the pool gave up: {other:?}"),
    }
}

fn verdict_for(nodes: &[(&str, RpcSource)], from: &str) -> SimVerdict {
    verdict_of_envelope(&corroborated_simulation(nodes, &drain_params(from)), from)
}

/// The confirm's friction for a transaction read in full with this verdict.
fn posture_of(verdict: &SimVerdict) -> vela_core::app::sign_confirm::Posture {
    use vela_core::app::clear_signing::{ClearSigning, ClearSurface};
    use vela_core::app::sign_request::SignRequest;
    let sign = DomainDriver::<SignRequest>::new().view();
    let mut clear = DomainDriver::<ClearSigning>::new().view();
    clear.resolving = false;
    clear.surface = ClearSurface::ClearSign;
    vela_core::app::sign_confirm::posture(&sign, &clear, Some(verdict))
}

#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn a_lying_rpc_never_gives_a_drain_a_plain_verdict() {
    if std::env::var("VELA_MULTISIG_FORK").is_err() {
        return;
    }
    let from = vela_core::dev_fixtures::account(0).unwrap().address;
    let honest = Proxy::start(&fork_url(), Kind::Honest);
    let honest2 = Proxy::start(&fork_url(), Kind::Honest);
    let liar = Proxy::start(&fork_url(), Kind::Lying);

    // Two honest nodes: the drain is seen, believed, and plain.
    eprintln!("honest + honest");
    let truth = verdict_for(
        &[
            (&honest.url, RpcSource::Default),
            (&honest2.url, RpcSource::Public),
        ],
        &from,
    );
    assert_eq!(truth.kind, "deltas", "{truth:?}");
    assert_eq!(truth.source.as_deref(), Some("corroborated"));
    assert!(
        truth
            .deltas
            .iter()
            .any(|d| d.token.as_deref() == Some(&WXDAI.to_lowercase()) && d.delta.starts_with('-')),
        "the WXDAI leaves: {truth:?}"
    );
    let plain = posture_of(&truth);
    assert!(!plain.anyway && plain.hold_ms == 0, "{plain:?}");

    // The liar alone says "No asset changes": no verdict on its word.
    eprintln!("liar alone");
    let alone = verdict_for(&[(&liar.url, RpcSource::Default)], &from);
    assert_eq!(alone.kind, "unconfirmed", "{alone:?}");
    assert_eq!(alone.no_change_key, None, "never \"No asset changes\"");

    // The liar beside an honest node, asked first or second: they disagree.
    for nodes in [
        [
            (liar.url.as_str(), RpcSource::Default),
            (honest.url.as_str(), RpcSource::Public),
        ],
        [
            (honest.url.as_str(), RpcSource::Default),
            (liar.url.as_str(), RpcSource::Public),
        ],
    ] {
        eprintln!("{nodes:?}");
        let got = verdict_for(&nodes, &from);
        assert_eq!(got.kind, "unconfirmed", "{got:?}");
        assert_eq!(
            got.notice_key.as_deref(),
            Some("componentsUi.signing.simDisagreed")
        );
        assert_eq!(got.no_change_key, None);
        let held = posture_of(&got);
        assert!(held.anyway, "never a plain verdict: {held:?}");
        assert_eq!(
            held.hold_ms,
            vela_core::app::sign_confirm::HOLD_TO_CONFIRM_MS
        );
    }
}
