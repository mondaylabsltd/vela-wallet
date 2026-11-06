//! A simulated shell for the multisig machines: the genesis service and its
//! mailbox, chains holding Safe's own code, deployed Safes with their
//! storage, a store, a clock and passkeys signed by the parallel space's
//! fixture keys. Every [`Op`] is answered from here, so a test drives a
//! machine end to end with no I/O — the web tests' seams, turned into one
//! world a test sets up and then reads back.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{json, Value};
use vela_core::app::multisig_shell::{
    LocalAccount, MultisigOperation as Op, MultisigShellResult as Res, Notice, OwnerKey,
    SendPurpose,
};
use vela_core::app::SplitEffect;
use vela_core::multisig;

use super::support::DomainDriver;

pub const SERVICE: &str = "http://service.test";
pub const T0: f64 = 1_793_000_000_000.0;

pub const FACTORY: &str = "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67";
pub const SAFE_141: &str = "0x41675C099F32341bf84BFc5382aF534df5C7461a";
pub const SAFE_L2_141: &str = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762";
pub const HANDLER_141: &str = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99";
pub const TO_L2_SETUP_141: &str = "0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54";

fn code(name: &str) -> String {
    // This crate's own tests, or a shell that drives the machines through
    // this same world from its own crate (the desktop's scenario tests
    // include this file): the vectors stay here either way.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let candidates = [
        format!("{manifest}/tests/vectors/safe-code/{name}.hex"),
        format!("{manifest}/../../rust/crates/vela-core/tests/vectors/safe-code/{name}.hex"),
    ];
    let path = candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .unwrap_or(&candidates[0]);
    std::fs::read_to_string(path).unwrap().trim().to_owned()
}

/// Safe's own v1.4.1 code at its own addresses: what every chain here holds.
pub fn canonical_codes() -> BTreeMap<String, String> {
    [
        (FACTORY, "safe-proxy-factory-1.4.1"),
        (SAFE_141, "safe-1.4.1"),
        (SAFE_L2_141, "safe-l2-1.4.1"),
        (HANDLER_141, "compatibility-fallback-handler-1.4.1"),
        (TO_L2_SETUP_141, "safe-to-l2-setup-1.4.1"),
    ]
    .into_iter()
    .map(|(a, n)| (a.to_ascii_lowercase(), code(n)))
    .collect()
}

/// A deployed Safe's state on one chain.
#[derive(Clone, Debug)]
pub struct DeployedSafe {
    pub owners: Vec<String>,
    pub threshold: u64,
    pub nonce: u64,
    pub modules: Vec<String>,
    pub guard: Option<String>,
    pub handler: String,
    pub singleton: String,
    /// `approvedHashes[owner][hash]` set on chain (lowercase).
    pub approved: BTreeSet<(String, String)>,
}

impl DeployedSafe {
    pub fn clean(owners: &[&str], threshold: u64) -> Self {
        Self {
            owners: owners.iter().map(|o| (*o).to_owned()).collect(),
            threshold,
            nonce: 0,
            modules: Vec::new(),
            guard: None,
            handler: HANDLER_141.to_owned(),
            singleton: SAFE_L2_141.to_owned(),
            approved: BTreeSet::new(),
        }
    }
}

fn word_addr(a: &str) -> String {
    format!("{:0>64}", a.trim_start_matches("0x").to_ascii_lowercase())
}

fn word_u(n: u64) -> String {
    format!("{n:064x}")
}

fn addresses_ret(list: &[String]) -> String {
    let mut out = String::from("0x");
    out.push_str(&word_u(32));
    out.push_str(&word_u(list.len() as u64));
    for a in list {
        out.push_str(&word_addr(a));
    }
    out
}

fn modules_ret(list: &[String]) -> String {
    let mut out = String::from("0x");
    out.push_str(&word_u(64));
    out.push_str(&word_addr("0x0000000000000000000000000000000000000001"));
    out.push_str(&word_u(list.len() as u64));
    for a in list {
        out.push_str(&word_addr(a));
    }
    out
}

pub type RpcHook = Box<dyn FnMut(u64, &str, &Value) -> Option<Res>>;
/// What `POST /api/proposals` answers for one submission instead of taking
/// it: `(status, body, Retry-After)`; `None` lets the next hook (or the
/// mailbox) answer.
pub type SubmitHook = Box<dyn FnMut(&Value) -> Option<(u16, Value, Option<String>)>>;

/// The PeerConnect rooms, as the shell's transport would hold them: every
/// room opened, every message sent, and what arrives on each (a test
/// pushes it; a receive with nothing to deliver waits).
#[derive(Default)]
pub struct PeerSim {
    pub next: u32,
    /// `(invite, pairing)` of each `PeerOpen`, in order.
    pub opens: Vec<(Option<String>, Option<String>)>,
    pub sent: Vec<(String, Value)>,
    /// `(room, end)` of each `PeerClose`.
    pub closed: Vec<(String, bool)>,
    pub inbox: BTreeMap<String, VecDeque<Res>>,
}

impl PeerSim {
    /// Deliver `message` on `room`.
    pub fn deliver(&mut self, room: &str, message: Value) {
        self.inbox
            .entry(room.to_owned())
            .or_default()
            .push_back(Res::PeerMessage { message });
    }

    /// Close `room` from the other side.
    pub fn close(&mut self, room: &str, reason: &str) {
        self.inbox
            .entry(room.to_owned())
            .or_default()
            .push_back(Res::PeerClosed {
                reason: reason.to_owned(),
            });
    }

    /// What was sent on `room`, in order.
    pub fn sent_on(&self, room: &str) -> Vec<Value> {
        self.sent
            .iter()
            .filter(|(r, _)| r == room)
            .map(|(_, m)| m.clone())
            .collect()
    }
}

/// An EOA from a 32-byte seed: its key and its address (lowercase).
pub fn eoa(seed: u8) -> (k256::ecdsa::SigningKey, String) {
    let key = k256::ecdsa::SigningKey::from_slice(&[seed; 32]).unwrap();
    let point = key.verifying_key().to_sec1_point(false);
    let hash = vela_core::primitives::keccak256(&point.as_bytes()[1..]);
    (key, format!("0x{}", hex_of(&hash[12..])))
}

pub fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `r ‖ s ‖ v` (v 27/28) over a 32-byte digest.
pub fn eoa_sign(key: &k256::ecdsa::SigningKey, digest: &[u8]) -> String {
    let (sig, id) = key.sign_prehash_recoverable(digest);
    let mut out = sig.to_bytes().to_vec();
    out.push(27 + id.to_byte());
    format!("0x{}", hex_of(&out))
}

/// An injected wallet (EIP-1193) holding EOA keys: it signs typed data by
/// its own EIP-712 hash and `personal_sign` by EIP-191, as a real one does.
pub struct WalletSim {
    /// Lowercase.
    pub accounts: Vec<String>,
    pub chain_id: u64,
    pub keys: BTreeMap<String, k256::ecdsa::SigningKey>,
    /// It can sign typed data.
    pub typed: bool,
    /// Signs with this account's key whoever is asked (a wrong signer).
    pub signer: Option<String>,
    /// It moves to the network asked for.
    pub switches: bool,
    /// The next answers, by method, before the wallet's own.
    pub next: VecDeque<(String, Res)>,
    /// It has not answered yet: requests wait.
    pub hold: bool,
    pub log: Vec<(String, Value)>,
    pub sent: Vec<Value>,
}

impl WalletSim {
    pub fn holding(seeds: &[u8], chain_id: u64) -> Self {
        let mut keys = BTreeMap::new();
        let mut accounts = Vec::new();
        for seed in seeds {
            let (key, address) = eoa(*seed);
            accounts.push(address.clone());
            keys.insert(address, key);
        }
        Self {
            accounts,
            chain_id,
            keys,
            typed: true,
            signer: None,
            switches: true,
            next: VecDeque::new(),
            hold: false,
            log: Vec::new(),
            sent: Vec::new(),
        }
    }

    pub fn calls_of(&self, method: &str) -> usize {
        self.log.iter().filter(|(m, _)| m == method).count()
    }

    fn key_for(&self, account: &str) -> Option<&k256::ecdsa::SigningKey> {
        let who = self
            .signer
            .as_deref()
            .unwrap_or(account)
            .to_ascii_lowercase();
        self.keys.get(&who)
    }

    pub fn answer(&mut self, method: &str, params: &Value) -> Res {
        self.log.push((method.to_owned(), params.clone()));
        if let Some(at) = self.next.iter().position(|(m, _)| m == method) {
            return self.next.remove(at).unwrap().1;
        }
        let refused = |code: i64, message: &str| Res::WalletRefused {
            code: Some(code),
            message: message.to_owned(),
        };
        let ok = |v: Value| Res::WalletAnswered { result: v };
        match method {
            "eth_chainId" => ok(json!(format!("0x{:x}", self.chain_id))),
            "eth_requestAccounts" | "eth_accounts" => ok(json!(self.accounts)),
            "wallet_switchEthereumChain" => {
                if !self.switches {
                    return refused(4902, "Unrecognized chain");
                }
                let id = params[0]["chainId"].as_str().unwrap();
                self.chain_id = u64::from_str_radix(id.trim_start_matches("0x"), 16).unwrap();
                ok(Value::Null)
            }
            "eth_signTypedData_v4" => {
                if !self.typed {
                    return refused(4200, "Method not supported");
                }
                let account = params[0].as_str().unwrap().to_owned();
                let digest =
                    vela_core::sign_message::original_hash(method, &params.to_string()).unwrap();
                match self.key_for(&account) {
                    Some(key) => ok(json!(eoa_sign(key, &digest))),
                    None => refused(4100, "Unauthorized"),
                }
            }
            "personal_sign" => {
                let account = params[1].as_str().unwrap().to_owned();
                let digest =
                    vela_core::sign_message::original_hash(method, &params.to_string()).unwrap();
                match self.key_for(&account) {
                    Some(key) => ok(json!(eoa_sign(key, &digest))),
                    None => refused(4100, "Unauthorized"),
                }
            }
            "eth_sendTransaction" => {
                self.sent.push(params[0].clone());
                ok(json!(format!("0x{}", "77".repeat(32))))
            }
            other => refused(4200, &format!("unexpected {other}")),
        }
    }
}

/// What an execution does once its user operation lands: the Safe's state
/// after it, and the transaction that carried it (its `ExecutionSuccess`).
#[derive(Clone, Debug)]
pub struct Landing {
    pub chain: u64,
    pub safe: String,
    pub after: DeployedSafe,
    pub safe_tx_hash: String,
    pub tx_hash: String,
    pub user_op_hash: String,
}

impl World {
    fn land(&mut self, l: Landing) {
        self.deploy(l.chain, &l.safe, l.after);
        self.receipts.insert(
            l.tx_hash.to_ascii_lowercase(),
            json!({"logs": [
                {"address": l.safe, "topics": [
                    "0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e",
                    l.safe_tx_hash.to_ascii_lowercase()
                ], "data": "0x"},
                {"address": "0x0000000071727De22E5E9d8BAf0edAc6f37da032", "topics": [
                    "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f",
                    l.user_op_hash
                ], "data": "0x"}
            ]}),
        );
        self.user_op = Some(("included".to_owned(), Some(l.tx_hash)));
    }
}

pub struct World {
    pub now: f64,
    pub store: BTreeMap<String, String>,
    // The genesis service.
    pub service_url: String,
    pub service_down: bool,
    pub home_chain: Option<u64>,
    pub served: Vec<u64>,
    pub records: BTreeMap<String, Value>,
    /// What `POST /api/import` answers (status, body); default: recorded.
    pub import_status: (u16, Value),
    pub imports: Vec<Value>,
    pub cross_chain: bool,
    pub mailbox: Vec<Value>,
    pub mailbox_down: BTreeSet<u64>,
    /// What `POST /api/proposals` answers instead of taking it.
    pub submit_status: Option<(u16, Value, Option<String>)>,
    /// Tried before `submit_status` and the mailbox, in order.
    pub submit_hooks: Vec<SubmitHook>,
    pub submits: Vec<Value>,
    pub tx_service: BTreeMap<String, (u16, Value)>,
    pub http_log: Vec<String>,
    // The chains.
    pub silent: BTreeSet<u64>,
    pub codes: BTreeMap<(u64, String), String>,
    pub canonical_missing: BTreeSet<(u64, String)>,
    pub safes: BTreeMap<(u64, String), DeployedSafe>,
    pub tx_counts: BTreeMap<(u64, String), u64>,
    pub rpc_log: Vec<(u64, String, Value)>,
    pub rpc_hooks: Vec<RpcHook>,
    // The person.
    pub locals: Vec<LocalAccount>,
    pub cancel_passkey: bool,
    pub risk_signs: bool,
    pub passkeys: Vec<(String, String)>,
    /// What each passkey prompt was for (`PasskeyPurpose`, as JSON), in order.
    pub passkey_purposes: Vec<Value>,
    pub risks_asked: Vec<Value>,
    pub sent: Vec<(u64, String, Vec<Value>)>,
    /// What each sent user operation was for, in order.
    pub purposes: Vec<Option<SendPurpose>>,
    pub send_answer: Option<Res>,
    pub quote_answer: Option<Res>,
    pub notices: Vec<Notice>,
    pub random: u8,
    /// What a sent user operation does on chain when it lands.
    pub on_send: Option<Landing>,
    /// The relay's word on a user operation: (status, tx hash).
    pub user_op: Option<(String, Option<String>)>,
    pub receipts: BTreeMap<String, Value>,
    pub balances: BTreeMap<(u64, String), u128>,
    pub sim_reply: Option<String>,
    /// Answered before `sim_reply`, one per simulation, in order.
    pub sim_replies: std::collections::VecDeque<Option<String>>,
    // The outside owner's side.
    pub peer: PeerSim,
    pub wallet: Option<WalletSim>,
    /// The passkey fails with this detail (neither signed nor cancelled).
    pub passkey_fails: Option<String>,
    /// What `eth_estimateGas` answers for any call (`None`: an RPC error).
    pub estimate_gas: Option<u64>,
    /// Accounts Vela's shared passkey signer is configured for, per chain
    /// (`getConfiguration` non-empty there).
    pub configured: BTreeSet<(u64, String)>,
    /// What `RecipientRisk` answers per address (lowercase): (contract, first time).
    pub recipients: BTreeMap<String, (Option<bool>, Option<bool>)>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            now: T0,
            store: BTreeMap::new(),
            service_url: SERVICE.to_owned(),
            service_down: false,
            home_chain: Some(100),
            served: vec![100],
            records: BTreeMap::new(),
            import_status: (200, json!({"status": "recorded", "recordedOn": [100]})),
            imports: Vec::new(),
            cross_chain: true,
            mailbox: Vec::new(),
            mailbox_down: BTreeSet::new(),
            submit_status: None,
            submit_hooks: Vec::new(),
            submits: Vec::new(),
            tx_service: BTreeMap::new(),
            http_log: Vec::new(),
            silent: BTreeSet::new(),
            codes: BTreeMap::new(),
            canonical_missing: BTreeSet::new(),
            safes: BTreeMap::new(),
            tx_counts: BTreeMap::new(),
            rpc_log: Vec::new(),
            rpc_hooks: Vec::new(),
            locals: Vec::new(),
            cancel_passkey: false,
            risk_signs: true,
            passkeys: Vec::new(),
            passkey_purposes: Vec::new(),
            risks_asked: Vec::new(),
            sent: Vec::new(),
            purposes: Vec::new(),
            send_answer: None,
            quote_answer: None,
            notices: Vec::new(),
            random: 7,
            on_send: None,
            user_op: None,
            receipts: BTreeMap::new(),
            balances: BTreeMap::new(),
            sim_reply: None,
            sim_replies: std::collections::VecDeque::new(),
            peer: PeerSim::default(),
            wallet: None,
            passkey_fails: None,
            estimate_gas: None,
            configured: BTreeSet::new(),
            recipients: BTreeMap::new(),
        }
    }
}

/// The parallel space's fixture account `index` as a Vela account here.
pub fn fixture(index: usize) -> LocalAccount {
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

/// A multisig genesis for these owners (the name "Quiet Harbor").
pub fn genesis(owners: &[&str], threshold: u64, salt: &str) -> multisig::MultisigGenesis {
    let owners: Vec<String> = owners.iter().map(|o| (*o).to_owned()).collect();
    multisig::multisig_genesis(&owners, threshold, "Quiet Harbor", salt).unwrap()
}

/// A stored row, as the web writes `vela.multisigAccounts`.
pub fn stored_row(g: &multisig::MultisigGenesis, chain_ids: &[u64], recorded: bool) -> Value {
    json!({
        "address": g.address, "genesis": g.genesis, "owners": g.owners,
        "threshold": g.threshold, "recorded": recorded, "chainIds": chain_ids, "addedMs": 1,
    })
}

impl World {
    pub fn record(&mut self, address: &str, genesis: &multisig::Genesis, pending: bool) {
        self.records.insert(
            address.to_ascii_lowercase(),
            json!({
                "found": true,
                "pending": pending,
                "genesis": genesis,
                "recordedOn": [100],
                "readFrom": 100,
            }),
        );
    }

    pub fn deploy(&mut self, chain: u64, address: &str, safe: DeployedSafe) {
        self.codes
            .insert((chain, address.to_ascii_lowercase()), "0x6080".to_owned());
        self.safes
            .insert((chain, address.to_ascii_lowercase()), safe);
    }

    pub fn safe_mut(&mut self, chain: u64, address: &str) -> &mut DeployedSafe {
        self.safes
            .get_mut(&(chain, address.to_ascii_lowercase()))
            .unwrap()
    }

    pub fn reads_on(&self, chain: u64) -> usize {
        self.rpc_log.iter().filter(|(c, _, _)| *c == chain).count()
    }

    pub fn stored_accounts(&self) -> Vec<Value> {
        serde_json::from_str(
            self.store
                .get("vela.multisigAccounts")
                .map_or("[]", String::as_str),
        )
        .unwrap()
    }

    fn code_at(&self, chain: u64, address: &str) -> String {
        let a = address.to_ascii_lowercase();
        if let Some(c) = self.codes.get(&(chain, a.clone())) {
            return c.clone();
        }
        if !self.canonical_missing.contains(&(chain, a.clone())) {
            if let Some(c) = canonical_codes().get(&a) {
                return c.clone();
            }
        }
        "0x".to_owned()
    }

    fn http(&mut self, method: &str, url: &str, body: Option<&str>) -> Res {
        self.http_log.push(format!("{method} {url}"));
        let answer = |status: u16, body: Value| Res::HttpAnswered {
            status,
            retry_after: None,
            body: body.to_string(),
        };
        if let Some(rest) = url.strip_prefix("https://api.safe.global/tx-service/") {
            return match self.tx_service.get(rest) {
                Some((s, b)) => answer(*s, b.clone()),
                None => answer(404, json!({})),
            };
        }
        let Some(path) = url.strip_prefix(&self.service_url) else {
            return Res::HttpUnreachable {
                detail: format!("no host for {url}"),
            };
        };
        if self.service_down {
            return Res::HttpUnreachable {
                detail: "connection refused".to_owned(),
            };
        }
        if path == "/api/health" {
            return answer(
                200,
                json!({
                    "service": "safe-genesis-registry",
                    "ok": true,
                    "homeChainId": self.home_chain,
                    "chains": self.served.iter().map(|c| json!({"chainId": c, "registry": *c == 100})).collect::<Vec<_>>(),
                }),
            );
        }
        if let Some(safe) = path.strip_prefix("/api/genesis/") {
            return match self.records.get(&safe.to_ascii_lowercase()) {
                Some(r) => answer(200, r.clone()),
                None => answer(404, json!({"found": false})),
            };
        }
        if path == "/api/import" && method == "POST" {
            let req: Value = serde_json::from_str(body.unwrap_or("{}")).unwrap();
            self.imports.push(req.clone());
            let (status, body) = self.import_status.clone();
            if status == 200 && body["status"] == "recorded" {
                if let (Some(safe), Some(g)) = (req["safe"].as_str(), req.get("genesis")) {
                    let g: multisig::Genesis = serde_json::from_value(g.clone()).unwrap();
                    self.record(safe, &g, false);
                }
            }
            return answer(status, body);
        }
        if path == "/api/proposals" && method == "POST" {
            let req: Value = serde_json::from_str(body.unwrap_or("{}")).unwrap();
            self.submits.push(req.clone());
            for hook in &mut self.submit_hooks {
                if let Some((status, body, retry)) = hook(&req) {
                    return Res::HttpAnswered {
                        status,
                        retry_after: retry,
                        body: body.to_string(),
                    };
                }
            }
            if let Some((status, body, retry)) = self.submit_status.clone() {
                return Res::HttpAnswered {
                    status,
                    retry_after: retry,
                    body: body.to_string(),
                };
            }
            return answer(201, self.take_submission(&req));
        }
        if let Some(rest) = path.strip_prefix("/api/proposals/") {
            let (safe, query) = rest.split_once('?').unwrap_or((rest, ""));
            let chain: Option<u64> = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("chainId="))
                .and_then(|c| c.parse().ok());
            let mine: Vec<Value> = self
                .mailbox
                .iter()
                .filter(|p| p["safe"].as_str().unwrap().eq_ignore_ascii_case(safe))
                .filter(|p| chain.is_none_or(|c| p["chainId"] == c))
                .cloned()
                .collect();
            if let Some(c) = chain {
                if self.mailbox_down.contains(&c) {
                    return answer(502, json!({"error": "down"}));
                }
                return answer(
                    200,
                    json!({"members": {"deployed": false, "owners": [], "threshold": "1", "nonce": "0"}, "proposals": mine}),
                );
            }
            if !self.cross_chain {
                return answer(404, json!({"error": "not found"}));
            }
            let mut counts: BTreeMap<u64, u64> = BTreeMap::new();
            for p in &mine {
                *counts.entry(p["chainId"].as_u64().unwrap()).or_default() += 1;
            }
            return answer(
                200,
                json!({
                    "total": mine.len(),
                    "chains": counts.iter().map(|(c, n)| json!({"chainId": c, "count": n})).collect::<Vec<_>>(),
                    "proposals": mine,
                }),
            );
        }
        if path.starts_with("/api/task/") {
            return answer(200, json!({"status": "done"}));
        }
        answer(404, json!({"error": "no route"}))
    }

    /// The mailbox merges an approval into its proposal (or opens one).
    fn take_submission(&mut self, req: &Value) -> Value {
        let chain = req["chainId"].as_u64().unwrap();
        let safe = req["safe"].as_str().unwrap().to_owned();
        let tx: multisig::SafeTx = serde_json::from_value(req["safeTx"].clone()).unwrap();
        let hash = multisig::safe_tx_hash(chain, &safe, &tx).unwrap();
        let mut approval = req["approval"].clone();
        approval["createdMs"] = json!(self.now as u64);
        let owner = approval["owner"].as_str().unwrap().to_ascii_lowercase();
        let same = |p: &Value| {
            p["chainId"] == chain
                && p["safeTxHash"]
                    .as_str()
                    .unwrap()
                    .eq_ignore_ascii_case(&hash)
        };
        let status;
        match self.mailbox.iter().position(same) {
            Some(i) => {
                let p = &mut self.mailbox[i];
                let list = p["approvals"].as_array_mut().unwrap();
                if list
                    .iter()
                    .any(|a| a["owner"].as_str().unwrap().eq_ignore_ascii_case(&owner))
                {
                    status = "unchanged";
                } else {
                    list.push(approval);
                    status = "approved";
                }
                p["updatedMs"] = json!(self.now as u64);
            }
            None => {
                self.mailbox.push(json!({
                    "chainId": chain,
                    "safe": safe,
                    "safeTxHash": hash,
                    "safeTx": req["safeTx"],
                    "proposer": req["approval"]["owner"],
                    "approvals": [approval],
                    "createdMs": self.now as u64,
                    "updatedMs": self.now as u64,
                }));
                status = "created";
            }
        }
        let proposal = self.mailbox.iter().find(|p| same(p)).cloned().unwrap();
        json!({"status": status, "proposal": proposal})
    }

    fn rpc(&mut self, chain: u64, method: &str, params: &Value) -> Res {
        self.rpc_log
            .push((chain, method.to_owned(), params.clone()));
        for hook in &mut self.rpc_hooks {
            if let Some(r) = hook(chain, method, params) {
                return r;
            }
        }
        if self.silent.contains(&chain) {
            return Res::RpcUnanswered {
                detail: "timed out".to_owned(),
            };
        }
        let ok = |v: Value| Res::RpcResult { result: v };
        match method {
            "eth_getCode" => ok(json!(self.code_at(chain, params[0].as_str().unwrap()))),
            "eth_getTransactionCount" => ok(json!(format!(
                "0x{:x}",
                self.tx_counts
                    .get(&(chain, params[0].as_str().unwrap().to_ascii_lowercase()))
                    .copied()
                    .unwrap_or(0)
            ))),
            "eth_getStorageAt" => {
                let a = params[0].as_str().unwrap().to_ascii_lowercase();
                let slot = params[1].as_str().unwrap();
                let Some(s) = self.safes.get(&(chain, a)) else {
                    return ok(json!(format!("0x{}", "0".repeat(64))));
                };
                let v = match slot {
                    multisig::GUARD_STORAGE_SLOT => s.guard.clone().unwrap_or_default(),
                    multisig::FALLBACK_HANDLER_STORAGE_SLOT => s.handler.clone(),
                    multisig::SINGLETON_STORAGE_SLOT => s.singleton.clone(),
                    _ => String::new(),
                };
                ok(json!(format!(
                    "0x{}",
                    word_addr(if v.is_empty() { "0x0" } else { &v })
                )))
            }
            "eth_call" => self.eth_call(chain, &params[0]),
            "eth_estimateGas" => match self.estimate_gas {
                Some(g) => ok(json!(format!("0x{g:x}"))),
                None => Res::RpcError {
                    code: Some(-32000),
                    message: "gas required exceeds allowance".to_owned(),
                    data: None,
                },
            },
            "eth_getBlockByNumber" => ok(json!({
                "number": "0x64",
                "timestamp": format!("0x{:x}", (self.now / 1000.0) as u64),
            })),
            "eth_getLogs" => {
                let topics = &params[0]["topics"];
                let hash = topics[1].as_str().unwrap_or("").to_ascii_lowercase();
                let logs: Vec<Value> = self
                    .receipts
                    .iter()
                    .filter(|(_, r)| {
                        r["logs"].as_array().unwrap().iter().any(|l| {
                            l["topics"][1]
                                .as_str()
                                .unwrap_or("")
                                .eq_ignore_ascii_case(&hash)
                        })
                    })
                    .map(|(tx, _)| json!({"transactionHash": tx, "blockNumber": "0x64"}))
                    .collect();
                ok(json!(logs))
            }
            "eth_getTransactionReceipt" => ok(self
                .receipts
                .get(&params[0].as_str().unwrap().to_ascii_lowercase())
                .cloned()
                .unwrap_or(Value::Null)),
            "eth_getBalance" => ok(json!(format!(
                "0x{:x}",
                self.balances
                    .get(&(chain, params[0].as_str().unwrap().to_ascii_lowercase()))
                    .copied()
                    .unwrap_or(10u128.pow(20))
            ))),
            _ => Res::RpcError {
                code: Some(-32601),
                message: format!("{method} not here"),
                data: None,
            },
        }
    }

    fn eth_call(&mut self, chain: u64, call: &Value) -> Res {
        let to = call["to"].as_str().unwrap().to_ascii_lowercase();
        let data = call["data"].as_str().unwrap_or("0x").to_ascii_lowercase();
        let ok = |v: String| Res::RpcResult { result: json!(v) };
        if to == multisig::GENESIS_REGISTRY.to_ascii_lowercase() {
            return Res::RpcError {
                code: Some(3),
                message: "execution reverted".to_owned(),
                data: None,
            };
        }
        // `getConfiguration(address)` on Vela's shared passkey signer: a key
        // for an account configured there, zeros for every other address.
        if to == vela_core::safe::WEBAUTHN_SIGNER.to_ascii_lowercase()
            && data.starts_with("0xc44b11f7")
        {
            let account = format!("0x{}", &data[10 + 24..10 + 64]);
            let word = |b: &str| b.repeat(32);
            return ok(if self.configured.contains(&(chain, account)) {
                format!("0x{}{}{}", word("1a"), word("2b"), "0".repeat(64))
            } else {
                format!("0x{}", "0".repeat(192))
            });
        }
        let Some(s) = self.safes.get(&(chain, to)) else {
            return ok("0x".to_owned());
        };
        match &data[..10.min(data.len())] {
            "0xa0e67e2b" => ok(addresses_ret(&s.owners)),
            "0xe75235b8" => ok(format!("0x{}", word_u(s.threshold))),
            "0xaffed0e0" => ok(format!("0x{}", word_u(s.nonce))),
            "0xcc2f8452" => ok(modules_ret(&s.modules)),
            // approvedHashes(address,bytes32)
            "0x7d832974" => {
                let owner = format!("0x{}", &data[10 + 24..10 + 64]);
                let hash = format!("0x{}", &data[10 + 64..10 + 128]);
                ok(format!(
                    "0x{}",
                    word_u(u64::from(s.approved.contains(&(owner, hash))))
                ))
            }
            _ => ok("0x".to_owned()),
        }
    }

    fn passkey(&mut self, owner: &str, challenge: &str) -> Res {
        self.passkeys.push((owner.to_owned(), challenge.to_owned()));
        if self.cancel_passkey {
            return Res::PasskeyCancelled;
        }
        if let Some(detail) = &self.passkey_fails {
            return Res::PasskeyFailed {
                detail: detail.clone(),
            };
        }
        let local = self
            .locals
            .iter()
            .find(|l| l.address.eq_ignore_ascii_case(owner))
            .unwrap();
        let account = vela_core::dev_fixtures::by_credential_id(&local.keys[0].credential_id)
            .unwrap()
            .unwrap();
        let bytes = vela_core::primitives::from_hex(challenge).unwrap();
        let a = vela_core::dev_fixtures::build_assertion(
            &account,
            &bytes,
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

    /// The shell's answer to one operation.
    pub fn answer(&mut self, op: &Op) -> Res {
        match op {
            Op::Http {
                method, url, body, ..
            } => self.http(method, url, body.as_deref()),
            Op::Rpc {
                chain_id,
                method,
                params,
                ..
            } => self.rpc(*chain_id, method, params),
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
                self.now += f64::from(*ms);
                Res::Waited
            }
            Op::Now => Res::Now { now_ms: self.now },
            Op::Random { bytes } => {
                self.random = self.random.wrapping_add(1);
                Res::Random {
                    hex: format!("{:02x}", self.random).repeat(*bytes as usize),
                }
            }
            Op::Passkey {
                owner,
                challenge,
                purpose,
                ..
            } => {
                self.passkey_purposes.push(json!(purpose));
                self.passkey(owner, challenge)
            }
            Op::AskRisk { findings, .. } => {
                self.risks_asked.push(json!(findings));
                Res::RiskAnswered {
                    signs: self.risk_signs,
                }
            }
            Op::SendCalls {
                chain_id,
                from,
                calls,
                purpose,
                ..
            } => {
                self.purposes.push(purpose.clone());
                if let Some(landing) = self.on_send.take() {
                    self.land(landing);
                }
                self.sent.push((
                    *chain_id,
                    from.clone(),
                    calls
                        .iter()
                        .map(|c| serde_json::to_value(c).unwrap())
                        .collect(),
                ));
                self.send_answer.clone().unwrap_or(Res::CallsSent {
                    user_op_hash: format!("0x{}", "ab".repeat(32)),
                    tx_hash: None,
                })
            }
            Op::QuoteCalls { .. } => self.quote_answer.clone().unwrap_or(Res::Quoted {
                quote: json!({"kind": "quoted", "fee": {"token": null, "amount": "0x1"}, "totalGas": "0x1", "executorDeployed": true}),
            }),
            Op::Simulate { .. } => Res::Simulated {
                reply: self
                    .sim_replies
                    .pop_front()
                    .unwrap_or_else(|| self.sim_reply.clone()),
            },
            Op::RecipientRisk { address, .. } => {
                let (is_contract, first_time) = self
                    .recipients
                    .get(&address.to_ascii_lowercase())
                    .copied()
                    .unwrap_or((Some(false), Some(false)));
                Res::RecipientFacts {
                    is_contract,
                    first_time,
                }
            }
            Op::UserOpStatus { .. } => match &self.user_op {
                Some((s, tx)) => Res::UserOpStatus {
                    status: Some(s.clone()),
                    tx_hash: tx.clone(),
                },
                None => Res::UserOpStatus {
                    status: Some("pending".to_owned()),
                    tx_hash: None,
                },
            },
            Op::Notice { notice } => {
                self.notices.push(notice.clone());
                Res::Ack
            }
            Op::PeerOpen {
                invite, pairing, ..
            } => {
                self.peer.opens.push((invite.clone(), pairing.clone()));
                if invite.as_deref().is_some_and(|i| i.contains("DAMAGED")) {
                    return Res::PeerClosed {
                        reason: "invalid invite".to_owned(),
                    };
                }
                self.peer.next += 1;
                let n = self.peer.next;
                Res::PeerOpened {
                    room: format!("room-{n}"),
                    invite: (invite.is_none() && pairing.is_none())
                        .then(|| format!("wss://relay#invite-{n}")),
                    pairing: Some(pairing.clone().unwrap_or_else(|| format!("record-{n}"))),
                }
            }
            Op::PeerSend { room, message } => {
                self.peer.sent.push((room.clone(), message.clone()));
                Res::Ack
            }
            Op::PeerReceive { room } => self
                .peer
                .inbox
                .get_mut(room)
                .and_then(VecDeque::pop_front)
                .expect("a receive answered with nothing to deliver"),
            Op::PeerClose { room, end, .. } => {
                self.peer.closed.push((room.clone(), *end));
                Res::Ack
            }
            Op::WalletRequest { method, params } => self
                .wallet
                .as_mut()
                .expect("no wallet in this world")
                .answer(method, params),
        }
    }
}

/// Answer every outstanding operation, oldest first, until none is left
/// (or `limit` answers were given — a machine that polls forever).
pub fn settle<A>(sut: &mut DomainDriver<A>, world: &mut World, limit: usize) -> usize
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: SplitEffect<Op = Op>,
{
    let mut answered = 0;
    while answered < limit {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding.first() else { break };
        let res = world.answer(op);
        sut.resolve(res);
        answered += 1;
    }
    answered
}

/// As [`settle`], with waits answered last: every other operation is
/// answered first, and an [`Op::Wait`] only when nothing else is outstanding
/// — a shell's clock, where a bound of seconds runs out after the reads it
/// bounds have answered (a machine racing a read against a wait).
pub fn settle_reads_first<A>(sut: &mut DomainDriver<A>, world: &mut World, limit: usize) -> usize
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: SplitEffect<Op = Op>,
{
    let mut answered = 0;
    while answered < limit {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| soonest_wait(&outstanding))
            .cloned()
        else {
            break;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
        answered += 1;
    }
    answered
}

/// The wait a clock ends first among `outstanding`: the shortest (r16-3: a
/// review's 1 s re-ask pause runs out long before its 40 s bound, whichever
/// was asked first).
pub fn soonest_wait(outstanding: &[Op]) -> Option<&Op> {
    outstanding
        .iter()
        .filter_map(|op| match op {
            Op::Wait { ms } => Some((*ms, op)),
            _ => None,
        })
        .min_by_key(|(ms, _)| *ms)
        .map(|(_, op)| op)
        .or_else(|| outstanding.first())
}

impl World {
    /// Whether the world can answer `op` now: a receive with nothing to
    /// deliver, and a wallet that holds its answer, wait.
    pub fn ready(&self, op: &Op) -> bool {
        match op {
            Op::PeerReceive { room } => self.peer.inbox.get(room).is_some_and(|q| !q.is_empty()),
            Op::WalletRequest { .. } => self.wallet.as_ref().is_some_and(|w| !w.hold),
            _ => true,
        }
    }
}

/// As [`settle`], answering the oldest operation the world can answer now
/// (a wait only when nothing else can be) and leaving the waiting ones (open
/// rooms, a wallet that holds) outstanding.
pub fn settle_ready<A>(sut: &mut DomainDriver<A>, world: &mut World, limit: usize) -> usize
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: SplitEffect<Op = Op>,
{
    let mut answered = 0;
    while answered < limit {
        let outstanding = sut.outstanding();
        // A shell's clock: a wait runs out after the reads it bounds.
        let Some(op) = outstanding
            .iter()
            .find(|op| world.ready(op) && !matches!(op, Op::Wait { .. }))
            .or_else(|| outstanding.iter().find(|op| world.ready(op)))
            .cloned()
        else {
            break;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
        answered += 1;
    }
    answered
}
