//! The multisig machines, headless, against the fork harness
//! (`scripts/onchain/multisig-fork/up.sh`): a Gnosis fork, the genesis
//! service with its mailbox. Create a 2-of-3 of the parallel space's three
//! accounts → propose a transfer → approve (twice in all) → execute — every
//! rule the core's, every read and write real; the shell is this file.
//!
//! The shell side: HTTP and JSON-RPC over a plain socket (the harness is
//! local, no TLS); passkeys signed by the fixture keys; the executor's user
//! operation stood in for by sending its calls from the executor's own
//! address on anvil (`anvil_impersonateAccount`) — the same calls, the same
//! `msg.sender`, without a bundler.
//!
//! Ignored by default; run with the harness up:
//!
//! ```sh
//! FORK_PORT=9245 GENESIS_PORT=9287 … scripts/onchain/multisig-fork/up.sh
//! VELA_MULTISIG_FORK=1 FORK_PORT=9245 GENESIS_PORT=9287 \
//!   cargo test -p vela-core --features crux,dev-fixtures --test multisig_fork -- --ignored --nocapture
//! ```

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod fork_proxy;
mod support;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_accounts::{self as accounts, CreateChoice, MultisigAccounts, Phase};
use vela_core::app::multisig_proposals::{
    self as proposals, ActionState, MultisigProposals, ProposalKind, ProposalStatus,
};
use vela_core::app::multisig_shell::{
    LocalAccount, MultisigAccount, MultisigOperation as Op, MultisigShellResult as Res, OwnerKey,
};
use vela_core::app::SplitEffect;

fn port(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(default)
}

/// One HTTP/1.1 request to a local server: (status, retry-after, body).
fn http(
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<(u16, Option<String>, String), String> {
    let rest = url.strip_prefix("http://").ok_or("not http")?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let mut s = TcpStream::connect(host).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, payload) = text.split_once("\r\n\r\n").ok_or("no head")?;
    let status: u16 = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or("no status")?;
    let header = |name: &str| {
        head.lines()
            .find(|l| l.to_ascii_lowercase().starts_with(&format!("{name}:")))
            .map(|l| l[name.len() + 1..].trim().to_owned())
    };
    let body = if header("transfer-encoding").is_some_and(|t| t.contains("chunked")) {
        let mut out = String::new();
        let mut rest = payload;
        while let Some((size, tail)) = rest.split_once("\r\n") {
            let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
            if n == 0 {
                break;
            }
            out.push_str(&tail[..n]);
            rest = &tail[n + 2..];
        }
        out
    } else {
        payload.to_owned()
    };
    Ok((status, header("retry-after"), body))
}

struct Shell {
    service: String,
    rpcs: BTreeMap<u64, String>,
    /// R22: per chain, the second, independent node a corroborated read is
    /// also asked of (an honest `fork_proxy` in front of the same anvil).
    witnesses: BTreeMap<u64, String>,
    store: BTreeMap<String, String>,
    locals: Vec<LocalAccount>,
    last_tx: Option<String>,
    log: Vec<String>,
}

impl Shell {
    fn rpc(&self, chain: u64, method: &str, params: Value) -> Result<Value, (Option<i64>, String)> {
        self.rpc_full(chain, method, params)
            .map_err(|(code, message, _)| (code, message))
    }

    /// The node's answer, its error's `data` kept (r8-4: the probe's payload).
    #[allow(clippy::type_complexity)]
    fn rpc_full(
        &self,
        chain: u64,
        method: &str,
        params: Value,
    ) -> Result<Value, (Option<i64>, String, Option<Value>)> {
        let url = self
            .rpcs
            .get(&chain)
            .ok_or((None, format!("no network {chain}"), None))?;
        let body =
            json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
        let (_, _, text) = http("POST", url, Some(&body)).map_err(|e| (None, e, None))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| (None, e.to_string(), None))?;
        if let Some(e) = v.get("error") {
            return Err((
                e["code"].as_i64(),
                e["message"].as_str().unwrap_or("").to_owned(),
                e.get("data").filter(|d| !d.is_null()).cloned(),
            ));
        }
        Ok(v["result"].clone())
    }

    /// R22: a corroborated read — the chain's node and its independent
    /// witness asked the same — as the one envelope the core compares.
    fn corroborated(&self, chain: u64, method: &str, params: &Value) -> Value {
        let body =
            json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
        let ask = |url: Option<&String>| {
            url.and_then(|u| fork_proxy::post(u, &body))
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        };
        let first = ask(self.rpcs.get(&chain));
        let witness = ask(self.witnesses.get(&chain));
        vela_core::app::corroborate::corroborated_envelope(first.as_ref(), witness.as_ref(), false)
    }

    /// The executor's user operation, stood in for: each call sent from its
    /// own address on anvil.
    fn send_calls(&mut self, chain: u64, from: &str, calls: &[vela_core::multisig::Call]) -> Res {
        self.rpc(chain, "anvil_impersonateAccount", json!([from]))
            .unwrap();
        self.rpc(
            chain,
            "anvil_setBalance",
            json!([from, "0x3635C9ADC5DEA00000"]),
        )
        .unwrap();
        let mut last = None;
        for call in calls {
            let tx = self
                .rpc(
                    chain,
                    "eth_sendTransaction",
                    json!([{"from": from, "to": call.to, "value": call.value, "data": call.data}]),
                )
                .unwrap();
            let hash = tx.as_str().unwrap().to_owned();
            let mut receipt = Value::Null;
            for _ in 0..20 {
                receipt = self
                    .rpc(chain, "eth_getTransactionReceipt", json!([hash]))
                    .unwrap();
                if !receipt.is_null() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            assert_eq!(receipt["status"], "0x1", "call to {} reverted", call.to);
            last = Some(hash);
        }
        self.rpc(chain, "anvil_stopImpersonatingAccount", json!([from]))
            .ok();
        self.last_tx = last;
        Res::CallsSent {
            user_op_hash: format!("0x{}", "00".repeat(32)),
            tx_hash: None,
        }
    }

    fn answer(&mut self, op: &Op) -> Res {
        match op {
            Op::Http {
                method, url, body, ..
            } => match http(method, url, body.as_deref()) {
                Ok((status, retry_after, body)) => {
                    self.log.push(format!("{method} {url} → {status}"));
                    Res::HttpAnswered {
                        status,
                        retry_after,
                        body,
                    }
                }
                Err(detail) => Res::HttpUnreachable {
                    detail,
                    refused: None,
                },
            },
            Op::Rpc {
                chain_id,
                method,
                params,
                corroborate: true,
                ..
            } => Res::RpcCorroborated {
                envelope: self.corroborated(*chain_id, method, params),
            },
            Op::Rpc {
                chain_id,
                method,
                params,
                ..
            } => match self.rpc_full(*chain_id, method, params.clone()) {
                Ok(result) => Res::RpcResult { result },
                Err((Some(code), message, data)) => Res::RpcError {
                    code: Some(code),
                    message,
                    data,
                },
                Err((None, detail, _)) => Res::RpcUnanswered { detail },
            },
            Op::LoadStore { keys } => Res::StoreLoaded {
                entries: keys
                    .iter()
                    .map(|k| (k.clone(), self.store.get(k).cloned()))
                    .collect(),
            },
            Op::WriteStore { key, value } => {
                match value {
                    Some(v) => self.store.insert(key.clone(), v.clone()),
                    None => self.store.remove(key),
                };
                Res::StoreWritten
            }
            Op::Wait { ms } => {
                std::thread::sleep(Duration::from_millis(u64::from(*ms).min(2_000)));
                Res::Waited
            }
            Op::Now => Res::Now {
                now_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as f64,
            },
            Op::Random { bytes } => {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let digest = vela_core::primitives::keccak256(&seed.to_be_bytes());
                Res::Random {
                    hex: vela_core::primitives::to_hex(&digest[..*bytes as usize], false),
                }
            }
            Op::Passkey {
                owner, challenge, ..
            } => {
                let local = self
                    .locals
                    .iter()
                    .find(|l| l.address.eq_ignore_ascii_case(owner))
                    .unwrap();
                let account =
                    vela_core::dev_fixtures::by_credential_id(&local.keys[0].credential_id)
                        .unwrap()
                        .unwrap();
                let a = vela_core::dev_fixtures::build_assertion(
                    &account,
                    &vela_core::primitives::from_hex(challenge).unwrap(),
                    vela_core::dev_fixtures::RP_ID,
                    vela_core::dev_fixtures::ORIGIN,
                )
                .unwrap();
                Res::Asserted {
                    credential_id: a.credential_id_hex,
                    authenticator_data: a.authenticator_data_hex,
                    client_data_json: a.client_data_json_hex,
                    signature: a.signature_der_hex,
                }
            }
            Op::AskRisk { .. } => Res::RiskAnswered { signs: false },
            Op::Simulate { .. } => Res::Simulated { reply: None },
            Op::RecipientRisk { .. } => Res::RecipientFacts {
                is_contract: None,
                first_time: None,
            },
            Op::QuoteCalls { .. } => Res::Quoted {
                quote: json!({"kind": "quoted", "fee": {"token": null, "amount": "0x1"}, "totalGas": "0x1", "executorDeployed": true}),
            },
            Op::SendCalls {
                chain_id,
                from,
                calls,
                ..
            } => self.send_calls(*chain_id, from, calls),
            Op::UserOpStatus { .. } => Res::UserOpStatus {
                status: Some("included".to_owned()),
                tx_hash: self.last_tx.clone(),
            },
            Op::Notice { .. } => Res::Ack,
            other => panic!("not answered here: {other:?}"),
        }
    }
}

/// R22: an honest, independent second node for chain 100 — a `fork_proxy`
/// on its own loopback port in front of the same anvil.
fn honest_witness(rpc: &str) -> BTreeMap<u64, String> {
    let proxy = fork_proxy::Proxy::start(rpc, fork_proxy::Kind::Honest);
    BTreeMap::from([(100, proxy.url)])
}

fn settle<A>(sut: &mut DomainDriver<A>, shell: &mut Shell)
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: SplitEffect<Op = Op>,
{
    // A shell runs its operations at once: a bound (a look's cap, a wait)
    // runs out after the reads it bounds have answered — so every other
    // operation is answered first, a wait only when nothing else is out.
    for _ in 0..5_000 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| outstanding.first())
            .cloned()
        else {
            return;
        };
        let res = shell.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    panic!("never settled");
}

fn fixture(index: usize) -> LocalAccount {
    let f = vela_core::dev_fixtures::account(index).unwrap();
    LocalAccount {
        address: f.address.clone(),
        name: f.name.to_owned(),
        keys: vec![OwnerKey {
            credential_id: f.credential_id_hex.clone(),
            public_key_hex: f.public_key_hex.clone(),
        }],
        route: Default::default(),
    }
}

fn balance_of(shell: &Shell, who: &str) -> u128 {
    let b = shell
        .rpc(100, "eth_getBalance", json!([who, "latest"]))
        .unwrap();
    u128::from_str_radix(b.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}

#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn create_two_of_three_propose_approve_twice_execute() {
    if std::env::var("VELA_MULTISIG_FORK").is_err() {
        eprintln!("VELA_MULTISIG_FORK not set: skipped");
        return;
    }
    let service = format!("http://127.0.0.1:{}", port("GENESIS_PORT", 8787));
    let rpc = format!("http://127.0.0.1:{}", port("FORK_PORT", 8545));
    let locals: Vec<LocalAccount> = (0..3).map(fixture).collect();
    let mut shell = Shell {
        service: service.clone(),
        witnesses: honest_witness(&rpc),
        rpcs: BTreeMap::from([(100, rpc)]),
        store: BTreeMap::new(),
        locals: locals.clone(),
        last_tx: None,
        log: Vec::new(),
    };

    // -- create (accounts machine) ---------------------------------------------------
    let mut acc = DomainDriver::<MultisigAccounts>::new();
    acc.dispatch(accounts::Event::Start {
        service_url: shell.service.clone(),
        accounts: locals.clone(),
        networks: vec![100],
    });
    settle(&mut acc, &mut shell);
    acc.dispatch(accounts::Event::NewDraft);
    settle(&mut acc, &mut shell);
    acc.dispatch(accounts::Event::Create {
        choice: CreateChoice {
            owners: locals.iter().map(|l| l.address.clone()).collect(),
            threshold: 2,
            name: "Fork Harbor".to_owned(),
            chain_id: 100,
            creator: locals[0].address.clone(),
            members: BTreeMap::new(),
        },
    });
    settle(&mut acc, &mut shell);
    let view = acc.view();
    assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
    let address = view.create.address.clone().unwrap();
    let row = &view.accounts[0];
    assert!(
        row.account.recorded,
        "recorded by the service: {:?}",
        shell.log
    );
    eprintln!("created {address}, recorded");
    let account: MultisigAccount = row.account.clone();

    // Another device finds it by its address alone.
    let mut other = DomainDriver::<MultisigAccounts>::new();
    let mut other_shell = Shell {
        service: service.clone(),
        rpcs: shell.rpcs.clone(),
        witnesses: shell.witnesses.clone(),
        store: BTreeMap::new(),
        locals: Vec::new(),
        last_tx: None,
        log: Vec::new(),
    };
    other.dispatch(accounts::Event::Start {
        service_url: service.clone(),
        accounts: Vec::new(),
        networks: vec![100],
    });
    settle(&mut other, &mut other_shell);
    other.dispatch(accounts::Event::Add {
        address: address.clone(),
        network: Some(100),
        accept_risks: false,
    });
    settle(&mut other, &mut other_shell);
    assert_eq!(
        other.view().add.outcome,
        Some(accounts::AddOutcome::Added {
            address: address.clone()
        }),
        "found by address, Verified by the counterfactual rule on the fork"
    );

    // Fund it: what it sends is its own.
    shell
        .rpc(
            100,
            "anvil_setBalance",
            json!([address, "0xDE0B6B3A7640000"]),
        )
        .unwrap();

    // -- propose, approve (proposals machine) ------------------------------------------
    let mut ms = DomainDriver::<MultisigProposals>::new();
    ms.dispatch(proposals::Event::Start {
        service_url: service.clone(),
        accounts: locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    });
    settle(&mut ms, &mut shell);
    ms.dispatch(proposals::Event::Follow {
        account: Some(account.clone()),
    });
    settle(&mut ms, &mut shell);
    let to = "0x00000000000000000000000000000000000000bb";
    ms.dispatch(proposals::Event::Draft {
        chain_id: 100,
        proposal: ProposalKind::Transfers {
            transfers: vec![vela_core::multisig::Transfer {
                token: None,
                to: to.to_owned(),
                amount: "1000".to_owned(),
            }],
        },
    });
    settle(&mut ms, &mut shell);
    assert!(ms.view().draft.is_some(), "{:?}", ms.view().actions);
    ms.dispatch(proposals::Event::ConfirmDraft {
        as_member: Some(locals[0].address.clone()),
    });
    settle(&mut ms, &mut shell);
    let ActionState::Done {
        safe_tx_hash: hash,
        kept: false,
    } = ms.view().actions["propose"].clone()
    else {
        panic!("{:?} {:?}", ms.view().actions, shell.log);
    };
    eprintln!("proposed {hash}");
    ms.dispatch(proposals::Event::Approve {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        owner: locals[1].address.clone(),
    });
    settle(&mut ms, &mut shell);
    let item = ms
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(
        item.approved, 2,
        "both approvals verified and counted: {item:?}"
    );
    assert_eq!(item.status, ProposalStatus::Ready);

    // -- execute ------------------------------------------------------------------------
    let executor = locals[2].address.clone();
    assert!(item
        .can_execute
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&executor)));
    ms.dispatch(proposals::Event::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: executor.clone(),
    });
    settle(&mut ms, &mut shell);
    let preview =
        ms.view().previews[&format!("100:{hash}:{}", executor.to_ascii_lowercase())].clone();
    assert_eq!(preview["kind"], "priced", "{preview}");
    let before = balance_of(&shell, to);
    // The press carries the figure the sheet drew (§24; round 16 leftover 1:
    // a press over no figure signs nothing).
    let shown_fee = preview["fee"]["amount"]
        .as_str()
        .and_then(vela_core::app::multisig_shell::quantity)
        .map(|fee| fee.to_string());
    assert!(shown_fee.is_some(), "a figure drawn: {preview}");
    ms.dispatch(proposals::Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor,
        shown_fee,
    });
    settle(&mut ms, &mut shell);
    let verdict = ms.view().executions[&format!("100:{hash}")].clone();
    assert_eq!(
        verdict["kind"],
        "executed",
        "{verdict} {:?}",
        ms.view().actions
    );
    let after = balance_of(&shell, to);
    assert_eq!(after - before, 1000, "the transfer landed");
    let item = ms
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.status, ProposalStatus::Done);
    eprintln!("executed in {}", verdict["txHash"]);
}

// ---------------------------------------------------------------------------
// R22 (§36) — a lying node beside an honest one never yields a plain review
// ---------------------------------------------------------------------------

/// A member change of a real 2-of-3 on the fork, reviewed with the review's
/// reads asked of (first node, witness) as given: the liar (`fork_proxy`)
/// rewrites the state diff to "only the nonce moves" and refuses the state
/// probe. Asked first or second, it never yields a plain verdict; two honest
/// nodes do.
#[test]
#[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
fn a_lying_node_never_gives_a_member_change_a_plain_review() {
    if std::env::var("VELA_MULTISIG_FORK").is_err() {
        eprintln!("VELA_MULTISIG_FORK not set: skipped");
        return;
    }
    let service = format!("http://127.0.0.1:{}", port("GENESIS_PORT", 8787));
    let rpc = format!("http://127.0.0.1:{}", port("FORK_PORT", 8545));
    let honest = fork_proxy::Proxy::start(&rpc, fork_proxy::Kind::Honest).url;
    let honest2 = fork_proxy::Proxy::start(&rpc, fork_proxy::Kind::Honest).url;
    let liar = fork_proxy::Proxy::start(&rpc, fork_proxy::Kind::Lying).url;
    let locals: Vec<LocalAccount> = (0..3).map(fixture).collect();
    let mut shell = Shell {
        service: service.clone(),
        rpcs: BTreeMap::from([(100, honest.clone())]),
        witnesses: BTreeMap::from([(100, honest2.clone())]),
        store: BTreeMap::new(),
        locals: locals.clone(),
        last_tx: None,
        log: Vec::new(),
    };
    let mut acc = DomainDriver::<MultisigAccounts>::new();
    acc.dispatch(accounts::Event::Start {
        service_url: service.clone(),
        accounts: locals.clone(),
        networks: vec![100],
    });
    settle(&mut acc, &mut shell);
    acc.dispatch(accounts::Event::NewDraft);
    settle(&mut acc, &mut shell);
    acc.dispatch(accounts::Event::Create {
        choice: CreateChoice {
            owners: locals.iter().map(|l| l.address.clone()).collect(),
            threshold: 2,
            name: "Truth Harbor".to_owned(),
            chain_id: 100,
            creator: locals[0].address.clone(),
            members: BTreeMap::new(),
        },
    });
    settle(&mut acc, &mut shell);
    let view = acc.view();
    assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
    let account: MultisigAccount = view.accounts[0].account.clone();
    eprintln!("created {}", account.address);

    let review = |shell: &mut Shell, first: &str, witness: &str| -> Value {
        shell.rpcs.insert(100, first.to_owned());
        shell.witnesses.insert(100, witness.to_owned());
        let mut ms = DomainDriver::<MultisigProposals>::new();
        ms.dispatch(proposals::Event::Start {
            service_url: service.clone(),
            accounts: locals.clone(),
            networks: vec![100],
            known: Vec::new(),
        });
        settle(&mut ms, shell);
        ms.dispatch(proposals::Event::Follow {
            account: Some(account.clone()),
        });
        settle(&mut ms, shell);
        ms.dispatch(proposals::Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::MemberChange {
                change: vela_core::multisig::MemberChange::Add {
                    owner: "0x000000000000000000000000000000000000dEaD".to_owned(),
                    threshold: None,
                },
            },
        });
        settle(&mut ms, shell);
        let draft = ms.view().draft.expect("a draft");
        draft.review.expect("a review")
    };

    let plain = review(&mut shell, &honest, &honest2);
    eprintln!(
        "honest + honest: state {} · sign {}",
        plain["state"], plain["sign"]
    );
    assert_eq!(plain["state"]["verdict"]["kind"], "declared", "{plain}");
    assert_eq!(
        plain["sign"],
        json!({"allowed": true, "anyway": false}),
        "{plain}"
    );

    for (first, witness, name) in [
        (&liar, &honest, "liar first"),
        (&honest, &liar, "liar second"),
    ] {
        let got = review(&mut shell, first, witness);
        eprintln!("{name}: state {} · sign {}", got["state"], got["sign"]);
        assert_ne!(
            got["state"]["verdict"]["kind"], "unchanged",
            "{name}: {got}"
        );
        assert_ne!(got["state"]["verdict"]["kind"], "declared", "{name}: {got}");
        assert_eq!(got["sign"]["anyway"], true, "{name}: never plain: {got}");
    }
}

// ---------------------------------------------------------------------------
// §31 — the parent-Safe guard against the real contracts (APPENDIX-PARENT)
// ---------------------------------------------------------------------------

mod parent {
    use super::*;
    use vela_core::app::parent_safe_guard::{ParentReason, SafeOwnerAnswer};
    use vela_core::app::sign_request::{
        Event as SignEvent, SignAccountRef, SignApproveOpts, SignDappIdentity, SignErrorKind,
        SignOperation as SignOp, SignRequest, SignResponsePayload, SignShellResult, SignSurface,
    };

    const SENTINEL: &str = "0x0000000000000000000000000000000000000001";
    const ATTACKER: &str = "0xbadbadbadbadbadbadbadbadbadbadbadbadbad0";

    fn w_addr(a: &str) -> String {
        format!("{:0>64}", a.trim_start_matches("0x").to_ascii_lowercase())
    }
    fn w_num(n: u64) -> String {
        format!("{n:064x}")
    }
    fn pad(body: &str) -> String {
        format!("{body:0<width$}", width = body.len().div_ceil(64) * 64)
    }

    /// `execTransaction(safe, 0, inner, CALL, 0, 0, 0, 0x0, 0x0, sig)` with the
    /// "approved by msg.sender" signature `{r = owner, s = 0, v = 1}` — what a
    /// site asks the owner's account to send (the disclosure's shape (a)).
    fn exec_v1(safe: &str, inner: &str, owner: &str) -> String {
        let body = inner.trim_start_matches("0x");
        let dpad = pad(body);
        let sig = format!("{}{}01", w_addr(owner), w_num(0));
        let spad = pad(&sig);
        let data_off = 32 * 10;
        let sig_off = data_off + 32 + dpad.len() as u64 / 2;
        format!(
            "0x6a761202{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            w_addr(safe),
            w_num(0),
            w_num(data_off),
            w_num(0),
            w_num(0),
            w_num(0),
            w_num(0),
            w_addr("0x0"),
            w_addr("0x0"),
            w_num(sig_off),
            w_num(body.len() as u64 / 2),
            dpad,
            w_num(sig.len() as u64 / 2),
            spad
        )
    }

    fn owners_of(shell: &Shell, safe: &str) -> Vec<String> {
        let r = shell
            .rpc(
                100,
                "eth_call",
                json!([{"to": safe, "data": "0xa0e67e2b"}, "latest"]),
            )
            .unwrap();
        let hex = r.as_str().unwrap().trim_start_matches("0x").to_owned();
        let n = usize::from_str_radix(&hex[64..128], 16).unwrap();
        (0..n)
            .map(|i| format!("0x{}", &hex[128 + i * 64 + 24..128 + (i + 1) * 64]))
            .collect()
    }

    /// The shell's answer to the sign machine's ownership read: a real
    /// `eth_call` to the fork (or to `rpc`, when given).
    fn read_owner(shell: &Shell, op: &SignOp) -> SignShellResult {
        let SignOp::ReadSafeOwner {
            id,
            chain_id,
            safe,
            data,
            ..
        } = op
        else {
            panic!("not a read: {op:?}");
        };
        // R22 (SEC-PARENT-3): the read is corroborated — the chain's node
        // and its independent witness.
        let envelope = shell.corroborated(
            u64::from(*chain_id),
            "eth_call",
            &json!([{"to": safe, "data": data}, "latest"]),
        );
        let answer_of = |e: &Value| match (e.get("result"), e.get("error")) {
            (Some(v), _) => SafeOwnerAnswer::Returned {
                data: v.as_str().unwrap_or("0x").to_owned(),
            },
            (None, Some(err)) if !err.is_null() => SafeOwnerAnswer::RpcError {
                code: err["code"].as_i64(),
                message: err["message"].as_str().unwrap_or("").to_owned(),
            },
            _ => SafeOwnerAnswer::Unreachable {
                detail: "no node answered".to_owned(),
            },
        };
        SignShellResult::SafeOwnerRead {
            id: id.clone(),
            chain_id: *chain_id,
            safe: safe.clone(),
            answer: answer_of(&envelope),
            witness: vela_core::app::corroborate::witness_of(&envelope).map(answer_of),
            own_node: None,
        }
    }

    fn sign_machine(account: &str) -> DomainDriver<SignRequest> {
        let mut sign = DomainDriver::<SignRequest>::new();
        sign.dispatch(SignEvent::NetworksChanged {
            chain_ids: vec![100],
        });
        sign.dispatch(SignEvent::AccountsChanged {
            accounts: vec![SignAccountRef {
                address: account.to_owned(),
                credential_id: "cred-0".to_owned(),
            }],
            active_index: 0,
        });
        sign
    }

    fn arrive(
        sign: &mut DomainDriver<SignRequest>,
        id: &str,
        method: &str,
        params: &Value,
    ) -> Vec<SignOp> {
        sign.dispatch(SignEvent::RequestArrived {
            id: id.to_owned(),
            method: method.to_owned(),
            params_json: params.to_string(),
            origin: "https://evil.example".to_owned(),
            transport_id: "wp-1".to_owned(),
            dedicated_transport: false,
            per_request_chain: Some(100),
            dapp: Some(SignDappIdentity {
                name: "evil.example".to_owned(),
                url: Some("https://evil.example".to_owned()),
            }),
            granted_address: None,
            requested_address: None,
            request_ts_ms: None,
            now_ms: 1_700_000_000_000.0,
            first_party: false,
        })
    }

    /// Refused for `safe` as `reason`; a tap signs nothing; the close answers
    /// the site -32603 parent_safe_blocked.
    fn assert_refused(sign: &mut DomainDriver<SignRequest>, safe: &str, reason: ParentReason) {
        let view = sign.view();
        let blocked = view.blocked.clone().expect("refused on the sheet");
        assert_eq!(
            blocked.parent.as_deref(),
            Some(safe.to_ascii_lowercase().as_str())
        );
        assert_eq!(blocked.reason, Some(reason));
        assert!(!view.confirm_gate_open);
        let ops = sign.dispatch(SignEvent::ApproveTapped {
            opts: SignApproveOpts::default(),
        });
        assert!(
            !ops.iter().any(|op| matches!(
                op,
                SignOp::SignAndSubmit { .. } | SignOp::CheckBundlerFunding { .. }
            )),
            "nothing signed: {ops:?}"
        );
        let ops = sign.dispatch(SignEvent::SwipeDismissed);
        assert!(
            ops.iter().any(|op| matches!(
                op,
                SignOp::SendResponse {
                    payload: SignResponsePayload::Err {
                        code: -32603,
                        kind: SignErrorKind::ParentSafeBlocked,
                        ..
                    },
                    ..
                }
            )),
            "{ops:?}"
        );
        assert_eq!(sign.view().surface, SignSurface::Hidden);
    }

    /// The disclosure's parent scenario, end to end on the fork: a 1-of-2
    /// multisig P whose member A alone meets the threshold.
    ///
    /// 1. Vela's own flow works: P proposes, A approves (1 of 1 needed), A
    ///    executes — the transfer lands.
    /// 2. The site's takeover — A sending `P.execTransaction(swapOwner(A →
    ///    attacker))` with the "approved by msg.sender" signature — IS a
    ///    takeover on these contracts (proved on a snapshot, sent from A's
    ///    address directly, then reverted).
    /// 3. Through Vela it is refused, never signed: from the multisig list,
    ///    and with no list after the real `isOwner` read; `approveHash` and a
    ///    `SafeMessage` under P's domain too; an RPC that cannot be read
    ///    refuses "couldn't check".
    #[test]
    #[ignore = "needs the fork harness (scripts/onchain/multisig-fork/up.sh) and VELA_MULTISIG_FORK=1"]
    fn parent_safe_takeover_is_refused_while_the_multisigs_own_flow_runs() {
        if std::env::var("VELA_MULTISIG_FORK").is_err() {
            eprintln!("VELA_MULTISIG_FORK not set: skipped");
            return;
        }
        let service = format!("http://127.0.0.1:{}", port("GENESIS_PORT", 8787));
        let rpc = format!("http://127.0.0.1:{}", port("FORK_PORT", 8545));
        let locals: Vec<LocalAccount> = (0..2).map(fixture).collect();
        let a = locals[0].address.clone();
        let mut shell = Shell {
            service: service.clone(),
            witnesses: honest_witness(&rpc),
            rpcs: BTreeMap::from([(100, rpc)]),
            store: BTreeMap::new(),
            locals: locals.clone(),
            last_tx: None,
            log: Vec::new(),
        };

        // -- the parent: a 1-of-2 of A and B ------------------------------------------
        let mut acc = DomainDriver::<MultisigAccounts>::new();
        acc.dispatch(accounts::Event::Start {
            service_url: service.clone(),
            accounts: locals.clone(),
            networks: vec![100],
        });
        settle(&mut acc, &mut shell);
        acc.dispatch(accounts::Event::NewDraft);
        settle(&mut acc, &mut shell);
        acc.dispatch(accounts::Event::Create {
            choice: CreateChoice {
                owners: locals.iter().map(|l| l.address.clone()).collect(),
                threshold: 1,
                name: "Parent Harbor".to_owned(),
                chain_id: 100,
                creator: a.clone(),
                members: BTreeMap::new(),
            },
        });
        settle(&mut acc, &mut shell);
        let view = acc.view();
        assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
        let parent = view.create.address.clone().unwrap();
        let account: MultisigAccount = view.accounts[0].account.clone();
        shell
            .rpc(
                100,
                "anvil_setBalance",
                json!([parent, "0xDE0B6B3A7640000"]),
            )
            .unwrap();
        eprintln!("parent {parent}: 1-of-2 of {a} and {}", locals[1].address);

        // -- 1. Vela's own flow: propose, approve, execute ------------------------------
        let mut ms = DomainDriver::<MultisigProposals>::new();
        ms.dispatch(proposals::Event::Start {
            service_url: service.clone(),
            accounts: locals.clone(),
            networks: vec![100],
            known: Vec::new(),
        });
        settle(&mut ms, &mut shell);
        ms.dispatch(proposals::Event::Follow {
            account: Some(account.clone()),
        });
        settle(&mut ms, &mut shell);
        let to = "0x00000000000000000000000000000000000000cc";
        ms.dispatch(proposals::Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::Transfers {
                transfers: vec![vela_core::multisig::Transfer {
                    token: None,
                    to: to.to_owned(),
                    amount: "777".to_owned(),
                }],
            },
        });
        settle(&mut ms, &mut shell);
        ms.dispatch(proposals::Event::ConfirmDraft {
            as_member: Some(a.clone()),
        });
        settle(&mut ms, &mut shell);
        let ActionState::Done {
            safe_tx_hash: hash, ..
        } = ms.view().actions["propose"].clone()
        else {
            panic!("{:?} {:?}", ms.view().actions, shell.log);
        };
        let item = ms
            .view()
            .items
            .into_iter()
            .find(|i| i.safe_tx_hash == hash)
            .unwrap();
        assert_eq!(
            item.status,
            ProposalStatus::Ready,
            "proposed and approved by A: {item:?}"
        );
        ms.dispatch(proposals::Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: a.clone(),
        });
        settle(&mut ms, &mut shell);
        let preview = ms.view().previews[&format!("100:{hash}:{}", a.to_ascii_lowercase())].clone();
        let shown_fee = preview["fee"]["amount"]
            .as_str()
            .and_then(vela_core::app::multisig_shell::quantity)
            .map(|fee| fee.to_string());
        let before = balance_of(&shell, to);
        ms.dispatch(proposals::Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: a.clone(),
            shown_fee,
        });
        settle(&mut ms, &mut shell);
        let verdict = ms.view().executions[&format!("100:{hash}")].clone();
        assert_eq!(
            verdict["kind"],
            "executed",
            "{verdict} {:?}",
            ms.view().actions
        );
        assert_eq!(
            balance_of(&shell, to) - before,
            777,
            "Vela's own execute landed"
        );
        eprintln!("own flow: executed in {}", verdict["txHash"]);

        // -- 2. the site's payload is a real takeover --------------------------------------
        let owners = owners_of(&shell, &parent);
        let at = owners
            .iter()
            .position(|o| o.eq_ignore_ascii_case(&a))
            .expect("A is an owner");
        let prev = if at == 0 {
            SENTINEL.to_owned()
        } else {
            owners[at - 1].clone()
        };
        let swap = format!(
            "0xe318b52b{}{}{}",
            w_addr(&prev),
            w_addr(&a),
            w_addr(ATTACKER)
        );
        let takeover = exec_v1(&parent, &swap, &a);
        let snapshot = shell.rpc(100, "evm_snapshot", json!([])).unwrap();
        shell.send_calls(
            100,
            &a,
            &[vela_core::multisig::Call {
                to: parent.clone(),
                value: "0x0".to_owned(),
                data: takeover.clone(),
            }],
        );
        let after = owners_of(&shell, &parent);
        assert!(
            after.iter().any(|o| o.eq_ignore_ascii_case(ATTACKER))
                && !after.iter().any(|o| o.eq_ignore_ascii_case(&a)),
            "signed once, the site's request hands the parent over: {after:?}"
        );
        shell.rpc(100, "evm_revert", json!([snapshot])).unwrap();
        assert!(
            owners_of(&shell, &parent)
                .iter()
                .any(|o| o.eq_ignore_ascii_case(&a)),
            "reverted"
        );
        eprintln!("takeover proved on a snapshot (reverted)");

        // -- 3. through Vela: refused ----------------------------------------------------------
        let attack = json!([{ "to": parent, "data": takeover, "value": "0x0" }]);

        // (a) no list: the real isOwner read on the fork says A owns P.
        let mut sign = sign_machine(&a);
        let ops = arrive(&mut sign, "r-a1", "eth_sendTransaction", &attack);
        let read = ops
            .iter()
            .find(|op| matches!(op, SignOp::ReadSafeOwner { .. }))
            .expect("an unknown Safe is read")
            .clone();
        let answered = read_owner(&shell, &read);
        sign.resolve_matching(|op| *op == read, answered);
        assert_refused(&mut sign, &parent, ParentReason::Owner);

        // (a) with the device's multisig list: refused at once, no read.
        sign.dispatch(SignEvent::MultisigsChanged {
            addresses: vec![parent.clone()],
        });
        let ops = arrive(&mut sign, "r-a2", "eth_sendTransaction", &attack);
        assert!(!ops
            .iter()
            .any(|op| matches!(op, SignOp::ReadSafeOwner { .. })));
        assert_refused(&mut sign, &parent, ParentReason::Member);

        // (b) approveHash of the parent's next transaction.
        let approve = json!([{ "to": parent, "data": format!("0xd4d9bdcd{}", "cd".repeat(32)), "value": "0x0" }]);
        arrive(&mut sign, "r-b", "eth_sendTransaction", &approve);
        assert_refused(&mut sign, &parent, ParentReason::Member);

        // (c) a SafeMessage under the parent's domain (a nested EIP-1271 approval).
        let message = json!({
            "types": {
                "EIP712Domain": [{"name": "chainId", "type": "uint256"}, {"name": "verifyingContract", "type": "address"}],
                "SafeMessage": [{"name": "message", "type": "bytes"}]
            },
            "primaryType": "SafeMessage",
            "domain": { "chainId": 100, "verifyingContract": parent },
            "message": { "message": format!("0x{}", "ab".repeat(32)) }
        });
        arrive(
            &mut sign,
            "r-c",
            "eth_signTypedData_v4",
            &json!([a, message.to_string()]),
        );
        assert_refused(&mut sign, &parent, ParentReason::Member);

        // isOwner unreadable: a node that does not answer refuses, never allows.
        let mut dead = Shell {
            service: service.clone(),
            rpcs: BTreeMap::from([(100, "http://127.0.0.1:9".to_owned())]),
            witnesses: BTreeMap::new(),
            store: BTreeMap::new(),
            locals: Vec::new(),
            last_tx: None,
            log: Vec::new(),
        };
        let mut sign = sign_machine(&a);
        let ops = arrive(&mut sign, "r-u", "eth_sendTransaction", &attack);
        let read = ops
            .iter()
            .find(|op| matches!(op, SignOp::ReadSafeOwner { .. }))
            .unwrap()
            .clone();
        let answered = read_owner(&dead, &read);
        dead.log.clear();
        sign.resolve_matching(|op| *op == read, answered);
        assert_refused(&mut sign, &parent, ParentReason::Unchecked);

        // And P is untouched: A is still its owner, no attacker.
        let owners = owners_of(&shell, &parent);
        assert!(owners.iter().any(|o| o.eq_ignore_ascii_case(&a)));
        assert!(!owners.iter().any(|o| o.eq_ignore_ascii_case(ATTACKER)));
        eprintln!(
            "refused: member, owner (isOwner on the fork), approveHash, SafeMessage, unchecked"
        );
    }
}
