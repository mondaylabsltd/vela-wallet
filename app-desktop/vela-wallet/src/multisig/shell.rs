//! The desktop's ONE executor for the core's multisig machines
//! (`vela_core::app::multisig_*`): every [`Op`] any of them asks — the
//! accounts, the proposals, the members and Vela's side of an outside
//! owner's session — is answered here, by exactly one [`Res`]. The web's
//! `shell/handler.ts`, in-process.
//!
//! This file draws nothing and decides nothing. It sorts each operation by
//! WHERE it is performed and reports what happened in the vocabulary's own
//! words: an HTTP answer as it came, a JSON-RPC reply as the node gave it, a
//! store key as it was. What to ask, when, how often and what an answer
//! means is the core's. The product's own pipelines — the passkey (in Vela,
//! or on the member's trusted signing page), the send and fee pipelines,
//! simulation, recipient facts, the relay's word on a user operation — come
//! in as [`Deps`], so a test answers them from a simulated world and the app
//! from `live.rs`; a PeerConnect room comes in as [`Rooms`].
//!
//! The failure contract is `executor::mod`'s: nothing fails outward. Every
//! arm answers with the variant the operation owes, on both paths, and a
//! dependency that reports a failure is answered with that operation's own
//! failure ([`failure_of`], the web's `operationFailure`).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use vela_core::app::multisig_shell::{
    MultisigOperation as Op, MultisigShellResult as Res, Notice, PasskeyPurpose, SendPurpose,
    SentCall,
};
use vela_core::multisig_review::SimCall;

/// An HTTP answer, as it came (any status).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    /// The `Retry-After` header's raw text, when there was one.
    pub retry_after: Option<String>,
    pub body: String,
}

/// A JSON-RPC reply through the wallet's pool: its `result`, or the node's
/// `error`. A transport failure (no node answered) is the `Err` beside it.
#[derive(Clone, Debug, PartialEq)]
pub enum RpcReply {
    Result(Value),
    /// The node's error, whole: its `data` too (a revert's bytes — the
    /// state-diff probe reads it, r8-4).
    Error {
        code: Option<i64>,
        message: String,
        data: Option<Value>,
    },
}

/// Why nothing answered a request (`Deps::http`'s `Err`): its detail, for
/// the log, and whether this test build's guard refused it before it left
/// (§28, r18-8) — never sent, never the network's answer, and said so to the
/// core (`HttpUnreachable.refused`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpFailure {
    pub detail: String,
    pub refused: bool,
}

impl HttpFailure {
    /// Nothing answered (refused connection, offline, timed out).
    #[must_use]
    pub fn unanswered(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            refused: false,
        }
    }

    /// This test build's guard refused it before it left (§28).
    #[must_use]
    pub fn refused(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            refused: true,
        }
    }
}

/// The facts the send flow knows of a recipient (`None`: unread).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecipientFacts {
    pub is_contract: Option<bool>,
    pub first_time: Option<bool>,
}

/// The app's pipelines a multisig operation reaches. Every method BLOCKS
/// (a socket, a ceremony) and is called off the main thread; every one
/// answers — a failure is its `Err` (or the vocabulary's own failure for the
/// ones that answer a [`Res`]), never a panic.
pub trait Deps: Send + Sync + 'static {
    /// One request; `Err` when nothing answered (refused, offline, timed
    /// out) — or this test build's guard refused it ([`HttpFailure::refused`]).
    fn http(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        timeout: Duration,
    ) -> Result<HttpReply, HttpFailure>;
    /// One call through the RPC pool; `Err` when no node answered (`bound`:
    /// past it nobody did). `person`: asked on the person's own word (their
    /// Try again, their press — `MultisigOperation::Rpc::person`), handed to
    /// the pool as the person's ask, which is always sent (r17-1 / r17-10).
    fn rpc(
        &self,
        chain_id: u64,
        method: &str,
        params: &Value,
        bound: Option<Duration>,
        person: bool,
    ) -> Result<RpcReply, String>;
    /// The store keys, as they are (`None`: absent). `Err`: no store.
    fn load(&self, keys: &[String]) -> Result<BTreeMap<String, Option<String>>, String>;
    /// One key written (`None` removes it). `Err`: not written.
    fn write(&self, key: &str, value: Option<&str>) -> Result<(), String>;
    fn now_ms(&self) -> f64;
    fn random(&self, bytes: usize) -> Vec<u8>;
    /// `owner` (a Vela account here) signs `challenge`: in Vela, or on the
    /// trusted page it signs on. Answered `asserted`, `page_approved`,
    /// `passkey_cancelled` or `passkey_failed`.
    fn passkey(&self, owner: &str, chain_id: u64, challenge: &str, purpose: &PasskeyPurpose)
    -> Res;
    /// `from` sends `calls` as one user operation. Answered `calls_sent`,
    /// `calls_failed` or `calls_cancelled`.
    fn send_calls(
        &self,
        chain_id: u64,
        from: &str,
        calls: &[SentCall],
        fee: Option<&Value>,
        purpose: Option<&SendPurpose>,
    ) -> Res;
    /// The fee pipeline's quote: `quoted` or `quote_failed`.
    fn quote_calls(&self, chain_id: u64, from: &str, calls: &[SentCall]) -> Res;
    /// The signing sheet's simulation envelope, or `None` (the pool gave up).
    fn simulate(&self, chain_id: u64, from: &str, calls: &[SimCall]) -> Option<String>;
    fn recipient_facts(&self, chain_id: u64, address: &str) -> RecipientFacts;
    /// The relay's word on a user operation (status, tx hash), or `None`.
    fn user_op_status(
        &self,
        chain_id: u64,
        user_op_hash: &str,
    ) -> Option<(Option<String>, Option<String>)>;
}

/// A PeerConnect room as the transport hands it out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    pub room: String,
    pub invite: Option<String>,
    pub pairing: Option<String>,
}

/// The next thing a room says: a message, or its close (why).
#[derive(Clone, Debug, PartialEq)]
pub enum Arrival {
    Message(Value),
    Closed(String),
}

/// The PeerConnect transport behind the `peer_*` operations (`peer.rs`).
/// Rooms only: what is sent and what an arrival means are the core's.
pub trait Rooms: Send + Sync + 'static {
    /// Create (`invite` and `pairing` absent), join (`invite`) or reopen
    /// (`pairing`) a room. `Err`: it could not be opened (why).
    fn open(
        &self,
        relay: Option<&str>,
        invite: Option<&str>,
        pairing: Option<&str>,
    ) -> Result<Opened, String>;
    fn send(&self, room: &str, message: &Value);
    /// BLOCKS until the next message or the room's close.
    fn receive(&self, room: &str) -> Arrival;
    /// Leave the room (`end`: the pairing ends for both sides).
    fn close(&self, room: &str, end: bool);
}

/// Work that blocks, answered once.
pub type Work = Box<dyn FnOnce() -> Res + Send>;

/// Where one operation is performed.
pub enum Perform {
    /// Answered here, now: a store key (a local file), the clock, random
    /// bytes, a request this shell has nothing to answer with.
    Now(Res),
    /// Blocks (a socket, a ceremony, a room's next message): run on a
    /// thread of its own, answered when it returns.
    Blocking(Work),
    /// Answered after `Duration` (`wait`) — a timer, not a parked thread.
    After(Duration, Res),
    /// The person is asked (`ask_risk`): the screen answers
    /// `risk_answered`, and `false` when it cannot ask.
    AskRisk {
        chain_id: u64,
        safe: String,
        findings: Vec<Value>,
    },
    /// Routed by the host (`machines::route`), then answered `ack`.
    Notice(Notice),
}

impl std::fmt::Debug for Perform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Now(res) => f.debug_tuple("Now").field(res).finish(),
            Self::Blocking(_) => f.write_str("Blocking"),
            Self::After(delay, res) => f.debug_tuple("After").field(delay).field(res).finish(),
            Self::AskRisk { chain_id, safe, .. } => f
                .debug_struct("AskRisk")
                .field("chain_id", chain_id)
                .field("safe", safe)
                .finish_non_exhaustive(),
            Self::Notice(notice) => f.debug_tuple("Notice").field(notice).finish(),
        }
    }
}

impl Perform {
    /// Run it here, to completion (a test's synchronous shell): blocking
    /// work on this thread, a wait at once. The two the host owns —
    /// [`Perform::AskRisk`] and [`Perform::Notice`] — are handed back.
    #[cfg(test)]
    #[allow(
        clippy::result_large_err,
        reason = "a test's synchronous shell: the operation handed back whole, once"
    )]
    pub fn run_inline(self) -> Result<Res, Self> {
        match self {
            Self::Now(res) | Self::After(_, res) => Ok(res),
            Self::Blocking(work) => Ok(work()),
            other => Err(other),
        }
    }
}

/// The failure an operation answers when it cannot be performed — the
/// result the machine waiting on it reads as "it did not happen", never a
/// hang (the web's `operationFailure`).
#[must_use]
pub fn failure_of(op: &Op, detail: &str) -> Res {
    let detail = detail.to_owned();
    match op {
        Op::Http { .. } => Res::HttpUnreachable {
            detail,
            refused: None,
        },
        Op::Rpc { .. } => Res::RpcUnanswered { detail },
        Op::LoadStore { .. } | Op::WriteStore { .. } => Res::StoreFailed { detail },
        Op::Wait { .. } => Res::Waited,
        Op::Now => Res::Now {
            now_ms: crate::executor::now_ms(),
        },
        Op::Random { .. } => Res::Random {
            hex: "0x".to_owned(),
        },
        Op::Passkey { .. } => Res::PasskeyFailed { detail },
        Op::AskRisk { .. } => Res::RiskAnswered { signs: false },
        Op::SendCalls { .. } => Res::CallsFailed {
            detail,
            maybe_sent: false,
        },
        Op::QuoteCalls { .. } => Res::QuoteFailed { detail },
        Op::Simulate { .. } => Res::Simulated { reply: None },
        Op::RecipientRisk { .. } => Res::RecipientFacts {
            is_contract: None,
            first_time: None,
        },
        Op::UserOpStatus { .. } => Res::UserOpStatus {
            status: None,
            tx_hash: None,
        },
        Op::PeerOpen { .. } | Op::PeerReceive { .. } => Res::PeerClosed { reason: detail },
        Op::PeerSend { .. } | Op::PeerClose { .. } | Op::Notice { .. } => Res::Ack,
        Op::WalletRequest { .. } => Res::WalletRefused {
            code: None,
            message: detail,
        },
    }
}

/// `0x` + lowercase hex.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// The code a wallet refusal carries when there is no wallet to ask
/// (EIP-1193 `4900`, disconnected).
pub const NO_WALLET: i64 = 4900;

/// Sort `op` by where it is performed, binding the blocking ones to `deps`
/// and `rooms`. Pure: nothing is performed until the host runs it.
pub fn perform(op: &Op, deps: &Arc<dyn Deps>, rooms: &Arc<dyn Rooms>) -> Perform {
    match op {
        Op::Http {
            method,
            url,
            body,
            timeout_ms,
        } => {
            let (deps, method, url, body) =
                (deps.clone(), method.clone(), url.clone(), body.clone());
            let timeout = Duration::from_millis(u64::from(*timeout_ms));
            Perform::Blocking(Box::new(move || {
                match deps.http(&method, &url, body.as_deref(), timeout) {
                    Ok(reply) => Res::HttpAnswered {
                        status: reply.status,
                        retry_after: reply.retry_after,
                        body: reply.body,
                    },
                    // r18-8 (§28, §32): the guard's refusal is said as such,
                    // once — the core neither retries it nor calls it an outage.
                    Err(failure) => Res::HttpUnreachable {
                        detail: failure.detail,
                        refused: failure.refused.then_some(true),
                    },
                }
            }))
        }
        Op::Rpc {
            chain_id,
            method,
            params,
            bound_ms,
            person,
        } => {
            let (deps, method, params, chain_id, person) = (
                deps.clone(),
                method.clone(),
                params.clone(),
                *chain_id,
                *person,
            );
            let bound = bound_ms.map(|ms| Duration::from_millis(u64::from(ms)));
            Perform::Blocking(Box::new(move || {
                match deps.rpc(chain_id, &method, &params, bound, person) {
                    Ok(RpcReply::Result(result)) => Res::RpcResult { result },
                    // r8-4: the node's data reaches the core (the state probe reads it there).
                    Ok(RpcReply::Error {
                        code,
                        message,
                        data,
                    }) => Res::RpcError {
                        code,
                        message,
                        data,
                    },
                    Err(detail) => Res::RpcUnanswered { detail },
                }
            }))
        }
        // A local file: answered on this thread, in order with the writes
        // around it (`session.rs`' argument for the store).
        Op::LoadStore { keys } => Perform::Now(match deps.load(keys) {
            Ok(entries) => Res::StoreLoaded { entries },
            Err(detail) => Res::StoreFailed { detail },
        }),
        Op::WriteStore { key, value } => Perform::Now(match deps.write(key, value.as_deref()) {
            Ok(()) => Res::StoreWritten,
            Err(detail) => Res::StoreFailed { detail },
        }),
        Op::Wait { ms } => Perform::After(Duration::from_millis(u64::from(*ms)), Res::Waited),
        Op::Now => Perform::Now(Res::Now {
            now_ms: deps.now_ms(),
        }),
        Op::Random { bytes } => Perform::Now(Res::Random {
            hex: hex(&deps.random(*bytes as usize)),
        }),
        Op::Passkey {
            owner,
            chain_id,
            challenge,
            purpose,
        } => {
            let (deps, owner, challenge, purpose, chain_id) = (
                deps.clone(),
                owner.clone(),
                challenge.clone(),
                purpose.clone(),
                *chain_id,
            );
            Perform::Blocking(Box::new(move || {
                deps.passkey(&owner, chain_id, &challenge, &purpose)
            }))
        }
        Op::AskRisk {
            chain_id,
            safe,
            findings,
        } => Perform::AskRisk {
            chain_id: *chain_id,
            safe: safe.clone(),
            findings: findings.clone(),
        },
        Op::SendCalls {
            chain_id,
            from,
            calls,
            fee,
            purpose,
        } => {
            let (deps, from, calls, fee, purpose, chain_id) = (
                deps.clone(),
                from.clone(),
                calls.clone(),
                fee.clone(),
                purpose.clone(),
                *chain_id,
            );
            Perform::Blocking(Box::new(move || {
                deps.send_calls(chain_id, &from, &calls, fee.as_ref(), purpose.as_ref())
            }))
        }
        Op::QuoteCalls {
            chain_id,
            from,
            calls,
        } => {
            let (deps, from, calls, chain_id) =
                (deps.clone(), from.clone(), calls.clone(), *chain_id);
            Perform::Blocking(Box::new(move || deps.quote_calls(chain_id, &from, &calls)))
        }
        Op::Simulate {
            chain_id,
            from,
            calls,
            ..
        } => {
            let (deps, from, calls, chain_id) =
                (deps.clone(), from.clone(), calls.clone(), *chain_id);
            Perform::Blocking(Box::new(move || Res::Simulated {
                reply: deps.simulate(chain_id, &from, &calls),
            }))
        }
        Op::RecipientRisk { chain_id, address } => {
            let (deps, address, chain_id) = (deps.clone(), address.clone(), *chain_id);
            Perform::Blocking(Box::new(move || {
                let facts = deps.recipient_facts(chain_id, &address);
                Res::RecipientFacts {
                    is_contract: facts.is_contract,
                    first_time: facts.first_time,
                }
            }))
        }
        Op::UserOpStatus {
            chain_id,
            user_op_hash,
        } => {
            let (deps, hash, chain_id) = (deps.clone(), user_op_hash.clone(), *chain_id);
            Perform::Blocking(Box::new(move || {
                let (status, tx_hash) =
                    deps.user_op_status(chain_id, &hash).unwrap_or((None, None));
                Res::UserOpStatus { status, tx_hash }
            }))
        }
        Op::PeerOpen {
            relay,
            invite,
            pairing,
        } => {
            let (rooms, relay, invite, pairing) = (
                rooms.clone(),
                relay.clone(),
                invite.clone(),
                pairing.clone(),
            );
            Perform::Blocking(Box::new(move || {
                match rooms.open(relay.as_deref(), invite.as_deref(), pairing.as_deref()) {
                    Ok(opened) => Res::PeerOpened {
                        room: opened.room,
                        invite: opened.invite,
                        pairing: opened.pairing,
                    },
                    Err(reason) => Res::PeerClosed { reason },
                }
            }))
        }
        Op::PeerSend { room, message } => {
            rooms.send(room, message);
            Perform::Now(Res::Ack)
        }
        Op::PeerReceive { room } => {
            let (rooms, room) = (rooms.clone(), room.clone());
            Perform::Blocking(Box::new(move || match rooms.receive(&room) {
                Arrival::Message(message) => Res::PeerMessage { message },
                Arrival::Closed(reason) => Res::PeerClosed { reason },
            }))
        }
        Op::PeerClose { room, end, .. } => {
            // `walletpair_cancel` is the core's message, sent before this.
            rooms.close(room, *end);
            Perform::Now(Res::Ack)
        }
        // The desktop hosts no connect page: no wallet is injected here, so
        // a request to one is refused as EIP-1193 says a disconnected one is.
        Op::WalletRequest { .. } => Perform::Now(Res::WalletRefused {
            code: Some(NO_WALLET),
            message: "no wallet connected".to_owned(),
        }),
        Op::Notice { notice } => Perform::Notice(notice.clone()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use vela_core::app::multisig_shell::Notice;

    use super::*;

    /// A world for one operation at a time: each pipeline answers from a
    /// field, and every call is written down.
    #[derive(Default)]
    pub(super) struct Fake {
        pub http: Mutex<Option<Result<HttpReply, HttpFailure>>>,
        pub rpc: Mutex<Option<Result<RpcReply, String>>>,
        pub store: Mutex<BTreeMap<String, String>>,
        pub store_down: bool,
        pub passkey: Mutex<Option<Res>>,
        pub send: Mutex<Option<Res>>,
        pub quote: Mutex<Option<Res>>,
        pub sim: Option<String>,
        pub facts: RecipientFacts,
        pub status: Option<(Option<String>, Option<String>)>,
        pub log: Mutex<Vec<String>>,
        pub rooms: Mutex<Vec<String>>,
        pub arrival: Mutex<Option<Arrival>>,
        pub open: Mutex<Option<Result<Opened, String>>>,
    }

    impl Fake {
        fn note(&self, line: String) {
            self.log.lock().unwrap().push(line);
        }
    }

    impl Deps for Fake {
        fn http(
            &self,
            method: &str,
            url: &str,
            body: Option<&str>,
            timeout: Duration,
        ) -> Result<HttpReply, HttpFailure> {
            self.note(format!(
                "http {method} {url} {body:?} {}",
                timeout.as_millis()
            ));
            self.http.lock().unwrap().take().unwrap()
        }
        fn rpc(
            &self,
            chain_id: u64,
            method: &str,
            params: &Value,
            bound: Option<Duration>,
            person: bool,
        ) -> Result<RpcReply, String> {
            self.note(format!(
                "rpc {chain_id} {method} {params} {bound:?}{}",
                if person { " person" } else { "" }
            ));
            self.rpc.lock().unwrap().take().unwrap()
        }
        fn load(&self, keys: &[String]) -> Result<BTreeMap<String, Option<String>>, String> {
            if self.store_down {
                return Err("no store".to_owned());
            }
            let store = self.store.lock().unwrap();
            Ok(keys
                .iter()
                .map(|k| (k.clone(), store.get(k).cloned()))
                .collect())
        }
        fn write(&self, key: &str, value: Option<&str>) -> Result<(), String> {
            if self.store_down {
                return Err("read-only".to_owned());
            }
            let mut store = self.store.lock().unwrap();
            match value {
                Some(v) => store.insert(key.to_owned(), v.to_owned()),
                None => store.remove(key),
            };
            Ok(())
        }
        fn now_ms(&self) -> f64 {
            1_793_000_000_000.0
        }
        fn random(&self, bytes: usize) -> Vec<u8> {
            (0..bytes).map(|i| i as u8).collect()
        }
        fn passkey(
            &self,
            owner: &str,
            chain_id: u64,
            challenge: &str,
            purpose: &PasskeyPurpose,
        ) -> Res {
            self.note(format!(
                "passkey {owner} {chain_id} {challenge} {purpose:?}"
            ));
            self.passkey.lock().unwrap().take().unwrap()
        }
        fn send_calls(
            &self,
            chain_id: u64,
            from: &str,
            calls: &[SentCall],
            fee: Option<&Value>,
            purpose: Option<&SendPurpose>,
        ) -> Res {
            self.note(format!(
                "send {chain_id} {from} {} {fee:?} {}",
                calls.len(),
                purpose.is_some()
            ));
            self.send.lock().unwrap().take().unwrap()
        }
        fn quote_calls(&self, chain_id: u64, from: &str, calls: &[SentCall]) -> Res {
            self.note(format!("quote {chain_id} {from} {}", calls.len()));
            self.quote.lock().unwrap().take().unwrap()
        }
        fn simulate(&self, chain_id: u64, from: &str, calls: &[SimCall]) -> Option<String> {
            self.note(format!("simulate {chain_id} {from} {}", calls.len()));
            self.sim.clone()
        }
        fn recipient_facts(&self, chain_id: u64, address: &str) -> RecipientFacts {
            self.note(format!("recipient {chain_id} {address}"));
            self.facts
        }
        fn user_op_status(
            &self,
            chain_id: u64,
            user_op_hash: &str,
        ) -> Option<(Option<String>, Option<String>)> {
            self.note(format!("status {chain_id} {user_op_hash}"));
            self.status.clone()
        }
    }

    impl Rooms for Fake {
        fn open(
            &self,
            relay: Option<&str>,
            invite: Option<&str>,
            pairing: Option<&str>,
        ) -> Result<Opened, String> {
            self.rooms
                .lock()
                .unwrap()
                .push(format!("open {relay:?} {invite:?} {pairing:?}"));
            self.open.lock().unwrap().take().unwrap()
        }
        fn send(&self, room: &str, message: &Value) {
            self.rooms
                .lock()
                .unwrap()
                .push(format!("send {room} {message}"));
        }
        fn receive(&self, room: &str) -> Arrival {
            self.rooms.lock().unwrap().push(format!("receive {room}"));
            self.arrival.lock().unwrap().take().unwrap()
        }
        fn close(&self, room: &str, end: bool) {
            self.rooms
                .lock()
                .unwrap()
                .push(format!("close {room} {end}"));
        }
    }

    fn run(fake: Fake, op: &Op) -> (Res, Arc<Fake>) {
        let fake = Arc::new(fake);
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake.clone();
        let res = perform(op, &deps, &rooms)
            .run_inline()
            .unwrap_or_else(|p| panic!("not inline: {p:?}"));
        (res, fake)
    }

    fn tx() -> vela_core::multisig::SafeTx {
        serde_json::from_value(json!({
            "to": "0x0000000000000000000000000000000000000001",
            "value": "0", "data": "0x", "operation": 0, "safeTxGas": "0",
            "baseGas": "0", "gasPrice": "0",
            "gasToken": "0x0000000000000000000000000000000000000000",
            "refundReceiver": "0x0000000000000000000000000000000000000000",
            "nonce": "0"
        }))
        .unwrap()
    }

    #[test]
    fn http_answers_as_it_came_whatever_the_status() {
        let fake = Fake::default();
        *fake.http.lock().unwrap() = Some(Ok(HttpReply {
            status: 429,
            retry_after: Some("2".to_owned()),
            body: "{\"error\":\"busy\"}".to_owned(),
        }));
        let (res, fake) = run(
            fake,
            &Op::Http {
                method: "POST".to_owned(),
                url: "http://s/api/proposals".to_owned(),
                body: Some("{}".to_owned()),
                timeout_ms: 8000,
            },
        );
        assert_eq!(
            res,
            Res::HttpAnswered {
                status: 429,
                retry_after: Some("2".to_owned()),
                body: "{\"error\":\"busy\"}".to_owned()
            }
        );
        assert_eq!(
            fake.log.lock().unwrap()[0],
            "http POST http://s/api/proposals Some(\"{}\") 8000"
        );
    }

    #[test]
    fn http_nothing_answered_is_unreachable() {
        let fake = Fake::default();
        *fake.http.lock().unwrap() = Some(Err(HttpFailure::unanswered("connection refused")));
        let (res, _) = run(
            fake,
            &Op::Http {
                method: "GET".to_owned(),
                url: "http://s".to_owned(),
                body: None,
                timeout_ms: 1,
            },
        );
        assert_eq!(
            res,
            Res::HttpUnreachable {
                detail: "connection refused".to_owned(),
                refused: None,
            }
        );
    }

    /// r18-8 (§28, §32): a write this test build's guard refused reaches the
    /// core as `refused: Some(true)` — the core's cue to say it in the test
    /// build's words, ask once and never call it the service's outage.
    #[test]
    fn http_refused_by_the_guard_is_said_refused() {
        let fake = Fake::default();
        *fake.http.lock().unwrap() = Some(Err(HttpFailure::refused("guard: refused")));
        let (res, fake) = run(
            fake,
            &Op::Http {
                method: "POST".to_owned(),
                url: "https://genesis.getvela.app/api/import".to_owned(),
                body: Some("{}".to_owned()),
                timeout_ms: 1,
            },
        );
        assert_eq!(
            res,
            Res::HttpUnreachable {
                detail: "guard: refused".to_owned(),
                refused: Some(true),
            }
        );
        let log = fake.log.lock().unwrap().clone();
        assert_eq!(
            log.iter().filter(|l| l.starts_with("http ")).count(),
            1,
            "asked once: {log:?}"
        );
    }

    #[test]
    fn rpc_keeps_a_node_error_apart_from_no_answer() {
        let op = Op::Rpc {
            chain_id: 100,
            method: "eth_getCode".to_owned(),
            params: json!(["0x01", "latest"]),
            bound_ms: Some(4000),
            person: false,
        };
        let fake = Fake::default();
        *fake.rpc.lock().unwrap() = Some(Ok(RpcReply::Result(json!("0x6080"))));
        let (res, fake) = run(fake, &op);
        assert_eq!(
            res,
            Res::RpcResult {
                result: json!("0x6080")
            }
        );
        assert!(
            fake.log.lock().unwrap()[0].ends_with("Some(4s)"),
            "the bound is passed on"
        );

        let fake = Fake::default();
        *fake.rpc.lock().unwrap() = Some(Ok(RpcReply::Error {
            code: Some(-32000),
            message: "execution reverted".to_owned(),
            data: Some(json!("0x08c379a0")),
        }));
        assert_eq!(
            run(fake, &op).0,
            Res::RpcError {
                code: Some(-32000),
                message: "execution reverted".to_owned(),
                data: Some(json!("0x08c379a0")),
            }
        );

        let fake = Fake::default();
        *fake.rpc.lock().unwrap() = Some(Err("no answer in 4000 ms".to_owned()));
        assert_eq!(
            run(fake, &op).0,
            Res::RpcUnanswered {
                detail: "no answer in 4000 ms".to_owned()
            }
        );
    }

    /// r17-1 / r17-10: a read the core asks on the person's own word (their
    /// Try again, their press) reaches the pool as theirs; any other read
    /// does not.
    #[test]
    fn rpc_hands_the_persons_word_to_the_pool() {
        for person in [false, true] {
            let op = Op::Rpc {
                chain_id: 5_042_002,
                method: "eth_call".to_owned(),
                params: json!([]),
                bound_ms: None,
                person,
            };
            let fake = Fake::default();
            *fake.rpc.lock().unwrap() = Some(Ok(RpcReply::Result(json!("0x"))));
            let (_, fake) = run(fake, &op);
            assert_eq!(
                fake.log.lock().unwrap()[0].ends_with(" person"),
                person,
                "person = {person}"
            );
        }
    }

    #[test]
    fn the_store_is_read_and_written_in_place() {
        let fake = Arc::new(Fake::default());
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake.clone();
        let write = |value: Option<&str>| {
            perform(
                &Op::WriteStore {
                    key: "vela.multisigAccounts".to_owned(),
                    value: value.map(str::to_owned),
                },
                &deps,
                &rooms,
            )
        };
        assert!(matches!(
            write(Some("[1]")),
            Perform::Now(Res::StoreWritten)
        ));
        let Perform::Now(res) = perform(
            &Op::LoadStore {
                keys: vec![
                    "vela.multisigAccounts".to_owned(),
                    "vela.activeMultisig".to_owned(),
                ],
            },
            &deps,
            &rooms,
        ) else {
            panic!("a store read is answered in place");
        };
        assert_eq!(
            res,
            Res::StoreLoaded {
                entries: [
                    ("vela.activeMultisig".to_owned(), None),
                    ("vela.multisigAccounts".to_owned(), Some("[1]".to_owned())),
                ]
                .into_iter()
                .collect()
            }
        );
        assert!(matches!(write(None), Perform::Now(Res::StoreWritten)));
        assert!(
            fake.store.lock().unwrap().is_empty(),
            "a null value removes the key"
        );
    }

    #[test]
    fn a_store_that_cannot_be_used_says_so() {
        let fake = Fake {
            store_down: true,
            ..Fake::default()
        };
        let (res, _) = run(
            fake,
            &Op::LoadStore {
                keys: vec!["k".to_owned()],
            },
        );
        assert_eq!(
            res,
            Res::StoreFailed {
                detail: "no store".to_owned()
            }
        );
        let fake = Fake {
            store_down: true,
            ..Fake::default()
        };
        let (res, _) = run(
            fake,
            &Op::WriteStore {
                key: "k".to_owned(),
                value: Some("v".to_owned()),
            },
        );
        assert!(matches!(res, Res::StoreFailed { .. }));
    }

    #[test]
    fn a_wait_is_a_timer_and_the_clock_and_random_are_answered_here() {
        let fake = Arc::new(Fake::default());
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake;
        let Perform::After(delay, Res::Waited) = perform(&Op::Wait { ms: 1500 }, &deps, &rooms)
        else {
            panic!("a wait is a timer");
        };
        assert_eq!(delay, Duration::from_millis(1500));
        assert!(matches!(
            perform(&Op::Now, &deps, &rooms),
            Perform::Now(Res::Now { now_ms }) if now_ms == 1_793_000_000_000.0
        ));
        assert!(matches!(
            perform(&Op::Random { bytes: 3 }, &deps, &rooms),
            Perform::Now(Res::Random { hex }) if hex == "0x000102"
        ));
    }

    #[test]
    fn a_passkey_is_the_pipelines_word_in_the_vocabulary() {
        for answer in [
            Res::PasskeyCancelled,
            Res::PasskeyFailed {
                detail: "venue_blocked".to_owned(),
            },
            Res::PageApproved {
                approval: json!({"owner": "0x1"}),
            },
            Res::Asserted {
                credential_id: "cred".to_owned(),
                authenticator_data: "0x01".to_owned(),
                client_data_json: "0x02".to_owned(),
                signature: "0x03".to_owned(),
            },
        ] {
            let fake = Fake::default();
            *fake.passkey.lock().unwrap() = Some(answer.clone());
            let (res, fake) = run(
                fake,
                &Op::Passkey {
                    owner: "0xAbC".to_owned(),
                    chain_id: 100,
                    challenge: "0xfeed".to_owned(),
                    purpose: PasskeyPurpose::Approve {
                        multisig: "0xSafe".to_owned(),
                        safe_tx_hash: "0xhash".to_owned(),
                        name: None,
                        tx: Box::new(tx()),
                        anyway: false,
                        owners: Vec::new(),
                        threshold: 0,
                    },
                },
            );
            assert_eq!(res, answer);
            assert!(fake.log.lock().unwrap()[0].starts_with("passkey 0xAbC 100 0xfeed Approve"));
        }
    }

    #[test]
    fn a_risk_is_asked_of_the_person_never_answered_here() {
        let fake = Arc::new(Fake::default());
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake;
        let op = Op::AskRisk {
            chain_id: 100,
            safe: "0xSafe".to_owned(),
            findings: vec![json!({"from": "chain"})],
        };
        let Perform::AskRisk {
            chain_id,
            safe,
            findings,
        } = perform(&op, &deps, &rooms)
        else {
            panic!("the screen asks");
        };
        assert_eq!(
            (chain_id, safe.as_str(), findings.len()),
            (100, "0xSafe", 1)
        );
        // A shell that cannot ask says no.
        assert_eq!(
            failure_of(&op, "closed"),
            Res::RiskAnswered { signs: false }
        );
    }

    #[test]
    fn send_and_quote_go_through_the_pipelines() {
        let calls = vec![
            serde_json::from_value::<SentCall>(json!({
                "to": "0x0000000000000000000000000000000000000002", "value": "0x0", "data": "0x"
            }))
            .unwrap(),
        ];
        let fake = Fake::default();
        *fake.send.lock().unwrap() = Some(Res::CallsFailed {
            detail: "0xuserop".to_owned(),
            maybe_sent: true,
        });
        let (res, fake) = run(
            fake,
            &Op::SendCalls {
                chain_id: 100,
                from: "0xExec".to_owned(),
                calls: calls.clone(),
                fee: Some(json!({"amount": "1"})),
                purpose: None,
            },
        );
        assert_eq!(
            res,
            Res::CallsFailed {
                detail: "0xuserop".to_owned(),
                maybe_sent: true
            },
            "a maybe-sent answer is passed on, never sent again"
        );
        assert!(fake.log.lock().unwrap()[0].starts_with("send 100 0xExec 1 Some"));

        let fake = Fake::default();
        *fake.quote.lock().unwrap() = Some(Res::QuoteFailed {
            detail: "no coin".to_owned(),
        });
        let (res, _) = run(
            fake,
            &Op::QuoteCalls {
                chain_id: 100,
                from: "0xExec".to_owned(),
                calls,
            },
        );
        assert_eq!(
            res,
            Res::QuoteFailed {
                detail: "no coin".to_owned()
            }
        );
    }

    #[test]
    fn simulation_facts_and_status_answer_none_when_unread() {
        let (res, _) = run(
            Fake::default(),
            &Op::Simulate {
                chain_id: 100,
                from: "0xSafe".to_owned(),
                calls: Vec::new(),
                state_overrides: None,
            },
        );
        assert_eq!(res, Res::Simulated { reply: None });
        let (res, _) = run(
            Fake {
                facts: RecipientFacts {
                    is_contract: Some(true),
                    first_time: None,
                },
                ..Fake::default()
            },
            &Op::RecipientRisk {
                chain_id: 100,
                address: "0x2".to_owned(),
            },
        );
        assert_eq!(
            res,
            Res::RecipientFacts {
                is_contract: Some(true),
                first_time: None
            }
        );
        let (res, _) = run(
            Fake::default(),
            &Op::UserOpStatus {
                chain_id: 100,
                user_op_hash: "0xop".to_owned(),
            },
        );
        assert_eq!(
            res,
            Res::UserOpStatus {
                status: None,
                tx_hash: None
            }
        );
        let (res, _) = run(
            Fake {
                status: Some((Some("included".to_owned()), Some("0xtx".to_owned()))),
                ..Fake::default()
            },
            &Op::UserOpStatus {
                chain_id: 100,
                user_op_hash: "0xop".to_owned(),
            },
        );
        assert_eq!(
            res,
            Res::UserOpStatus {
                status: Some("included".to_owned()),
                tx_hash: Some("0xtx".to_owned())
            }
        );
    }

    #[test]
    fn rooms_open_send_receive_and_close() {
        let fake = Fake::default();
        *fake.open.lock().unwrap() = Some(Ok(Opened {
            room: "room-1".to_owned(),
            invite: Some("ws://r#i".to_owned()),
            pairing: Some("pair".to_owned()),
        }));
        let (res, fake) = run(
            fake,
            &Op::PeerOpen {
                relay: Some("ws://127.0.0.1:1".to_owned()),
                invite: None,
                pairing: None,
            },
        );
        assert_eq!(
            res,
            Res::PeerOpened {
                room: "room-1".to_owned(),
                invite: Some("ws://r#i".to_owned()),
                pairing: Some("pair".to_owned())
            }
        );
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake.clone();
        assert!(matches!(
            perform(
                &Op::PeerSend {
                    room: "room-1".to_owned(),
                    message: json!({"a": 1})
                },
                &deps,
                &rooms
            ),
            Perform::Now(Res::Ack)
        ));
        *fake.arrival.lock().unwrap() = Some(Arrival::Message(json!("hi")));
        let res = perform(
            &Op::PeerReceive {
                room: "room-1".to_owned(),
            },
            &deps,
            &rooms,
        )
        .run_inline()
        .unwrap();
        assert_eq!(
            res,
            Res::PeerMessage {
                message: json!("hi")
            }
        );
        *fake.arrival.lock().unwrap() = Some(Arrival::Closed("left".to_owned()));
        let res = perform(
            &Op::PeerReceive {
                room: "room-1".to_owned(),
            },
            &deps,
            &rooms,
        )
        .run_inline()
        .unwrap();
        assert_eq!(
            res,
            Res::PeerClosed {
                reason: "left".to_owned()
            }
        );
        assert!(matches!(
            perform(
                &Op::PeerClose {
                    room: "room-1".to_owned(),
                    cancel: true,
                    end: true
                },
                &deps,
                &rooms
            ),
            Perform::Now(Res::Ack)
        ));
        assert_eq!(
            fake.rooms.lock().unwrap().as_slice(),
            [
                "open Some(\"ws://127.0.0.1:1\") None None",
                "send room-1 {\"a\":1}",
                "receive room-1",
                "receive room-1",
                "close room-1 true",
            ]
        );
    }

    #[test]
    fn a_room_that_cannot_open_is_closed_with_why() {
        let fake = Fake::default();
        *fake.open.lock().unwrap() = Some(Err("invalid invite".to_owned()));
        let (res, _) = run(
            fake,
            &Op::PeerOpen {
                relay: None,
                invite: Some("bad".to_owned()),
                pairing: None,
            },
        );
        assert_eq!(
            res,
            Res::PeerClosed {
                reason: "invalid invite".to_owned()
            }
        );
    }

    #[test]
    fn no_wallet_is_injected_on_the_desktop() {
        let (res, _) = run(
            Fake::default(),
            &Op::WalletRequest {
                method: "eth_accounts".to_owned(),
                params: json!([]),
            },
        );
        assert_eq!(
            res,
            Res::WalletRefused {
                code: Some(NO_WALLET),
                message: "no wallet connected".to_owned()
            }
        );
    }

    #[test]
    fn a_notice_is_the_hosts_to_route() {
        let fake = Arc::new(Fake::default());
        let deps: Arc<dyn Deps> = fake.clone();
        let rooms: Arc<dyn Rooms> = fake;
        let notice = Notice::MailboxTook {
            safe: "0xSafe".to_owned(),
        };
        let Perform::Notice(routed) = perform(
            &Op::Notice {
                notice: notice.clone(),
            },
            &deps,
            &rooms,
        ) else {
            panic!("routed by the host");
        };
        assert_eq!(routed, notice);
    }

    /// Every operation has a failure that never leaves its machine waiting.
    #[test]
    fn every_operation_has_a_failure_of_its_own() {
        let cases: Vec<(Op, &str)> = vec![
            (
                Op::Http {
                    method: "GET".into(),
                    url: "u".into(),
                    body: None,
                    timeout_ms: 1,
                },
                "http_unreachable",
            ),
            (
                Op::Rpc {
                    chain_id: 1,
                    method: "m".into(),
                    params: json!([]),
                    bound_ms: None,
                    person: false,
                },
                "rpc_unanswered",
            ),
            (Op::LoadStore { keys: vec![] }, "store_failed"),
            (
                Op::WriteStore {
                    key: "k".into(),
                    value: None,
                },
                "store_failed",
            ),
            (Op::Wait { ms: 1 }, "waited"),
            (Op::Now, "now"),
            (Op::Random { bytes: 1 }, "random"),
            (
                Op::SendCalls {
                    chain_id: 1,
                    from: "f".into(),
                    calls: vec![],
                    fee: None,
                    purpose: None,
                },
                "calls_failed",
            ),
            (
                Op::QuoteCalls {
                    chain_id: 1,
                    from: "f".into(),
                    calls: vec![],
                },
                "quote_failed",
            ),
            (
                Op::Simulate {
                    chain_id: 1,
                    from: "f".into(),
                    calls: vec![],
                    state_overrides: None,
                },
                "simulated",
            ),
            (
                Op::RecipientRisk {
                    chain_id: 1,
                    address: "a".into(),
                },
                "recipient_facts",
            ),
            (
                Op::UserOpStatus {
                    chain_id: 1,
                    user_op_hash: "h".into(),
                },
                "user_op_status",
            ),
            (
                Op::PeerOpen {
                    relay: None,
                    invite: None,
                    pairing: None,
                },
                "peer_closed",
            ),
            (Op::PeerReceive { room: "r".into() }, "peer_closed"),
            (
                Op::PeerSend {
                    room: "r".into(),
                    message: json!(1),
                },
                "ack",
            ),
            (
                Op::PeerClose {
                    room: "r".into(),
                    cancel: false,
                    end: false,
                },
                "ack",
            ),
            (
                Op::WalletRequest {
                    method: "m".into(),
                    params: json!([]),
                },
                "wallet_refused",
            ),
        ];
        for (op, kind) in cases {
            let res = serde_json::to_value(failure_of(&op, "x")).unwrap();
            assert_eq!(res["type"], kind, "{op:?}");
        }
    }
}
