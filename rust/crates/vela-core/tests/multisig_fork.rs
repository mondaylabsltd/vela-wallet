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
                Err(detail) => Res::HttpUnreachable { detail },
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
    ms.dispatch(proposals::Event::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor,
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
