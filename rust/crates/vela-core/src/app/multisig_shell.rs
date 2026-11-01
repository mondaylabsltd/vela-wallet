//! The one shell vocabulary every multisig machine speaks (F0.4), and the
//! reads they share.
//!
//! The web's multisig orchestration (`app-web/vela-wallet/src/lib/multisig`)
//! is async TypeScript over `fetch`, the RPC pool, `localStorage`, passkeys
//! and PeerConnect. Here the same rules run as Crux machines whose async
//! commands `await` a shell operation wherever the web awaited I/O:
//!
//! ```text
//!  event ─► machine ─► Command::new(async { io.rpc(..).await; io.http(..).await; … })
//!                         │                        ▲
//!                         ▼ MultisigOperation      │ MultisigShellResult
//!                       shell (fetch, pool, store, passkey, relay)
//! ```
//!
//! **One vocabulary for four machines** (`multisig_accounts`,
//! `multisig_proposals`, `multisig_members`, `multisig_outside`): a shell
//! writes ONE executor for [`MultisigOperation`] and drives every machine
//! with it. The operations are deliberately plain — an HTTP request, a
//! JSON-RPC call on a chain, a store key, a wait, the clock, random bytes, a
//! passkey, a question to the person, calls an account sends, a PeerConnect
//! room, an injected wallet — so that what to ask, when, how often and what
//! the answer means stays in the core.
//!
//! What this module holds besides the vocabulary is what more than one
//! machine reads with:
//! - [`Outage`] — `service-state.svelte.ts`' schedule for a service that is away;
//! - [`Service`] — `service.ts`: the genesis service and its proposal mailbox,
//!   with its retry rules (three tries, a 5xx tried again, a `429` waited out
//!   when short, a write's `429` handed back as [`ServiceFail::Busy`]);
//! - [`Chain`] — `chain.ts`: code, nonce and members read through the
//!   wallet's own RPC, never the service's;
//! - [`judge_on`] and friends — `authority.ts` / `judge.ts`: who can act for a
//!   multisig on a chain (§3.5), read here, decided by `crate::multisig`.
//!
//! Nothing here does I/O: every read is an operation the shell answers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use crux_core::capability::Operation;
use crux_core::command::CommandContext;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::multisig::{self, Call, Genesis, SafeTx};

// ---------------------------------------------------------------------------
// The vocabulary
// ---------------------------------------------------------------------------

/// What a signature on a passkey is for, so the shell can say so and route
/// it (the member's own signing page, if it signs on one).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigPasskeyPurpose")
)]
pub enum PasskeyPurpose {
    /// An approval of `safe_tx_hash` (`approval_challenge`). A member that
    /// reviews and signs on a trusted page gets the transaction itself
    /// (`tx`), which the page reads and hashes again.
    Approve {
        multisig: String,
        safe_tx_hash: String,
        name: Option<String>,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        tx: Box<SafeTx>,
        /// r9-10: the one review asks for "… anyway" (its `sign.anyway`) —
        /// a page signing it says so, in the danger style, as the app's
        /// word (it may add friction, never take it away).
        #[serde(default)]
        anyway: bool,
    },
    /// The proof asking the genesis service to record a new multisig
    /// (`sponsor_challenge`).
    Record { multisig: String },
}

/// One call an account sends (an executor's user operation, a backup's
/// `record`). Quantities hex.
pub type SentCall = Call;

/// What a [`MultisigOperation::SendCalls`] is for, so the shell can say so
/// on the sender's signing page and pick its pipeline — never told from
/// what else rides along.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigSendPurpose"))]
pub enum SendPurpose {
    /// The executor's own user operation running `tx` on `multisig`
    /// (`vela_multisigExecute`, r7-1/r7-3): ONE operation, no approval of
    /// its own asked first. Its trusted page reads what runs from the
    /// operation's bytes; this says what they do not carry — the
    /// transaction, the multisig, its name and how many approvals it needs.
    Execute {
        multisig: String,
        name: Option<String>,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold: u64,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        tx: Box<SafeTx>,
    },
    /// The Ethereum backup's `record` call for `multisig`: the signing
    /// sheet, as any first-party transaction.
    Record { multisig: String },
}

/// An operation a multisig machine asks of its shell. Every one is answered
/// by exactly one [`MultisigShellResult`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigOperation"))]
pub enum MultisigOperation {
    /// One HTTP request (the genesis service and its mailbox, a Safe
    /// transaction service). `body` is JSON text. The shell gives up after
    /// `timeout_ms` and answers `http_unreachable`; it never retries — the
    /// core does.
    Http {
        method: String,
        url: String,
        body: Option<String>,
        timeout_ms: u32,
    },
    /// One JSON-RPC call on `chain_id` through the wallet's own RPC pool.
    /// `bound_ms`: past it the shell answers `rpc_unanswered` (the web's
    /// `bounded`) — a read that never answers never holds a screen.
    Rpc {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        method: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        params: Value,
        bound_ms: Option<u32>,
    },
    /// Read these store keys (the web's `localStorage`).
    LoadStore { keys: Vec<String> },
    /// Write one store key; `None` removes it.
    WriteStore { key: String, value: Option<String> },
    /// Wait, without the core owning a clock.
    Wait { ms: u32 },
    /// The wall clock, Unix ms.
    Now,
    /// `bytes` random bytes (a new multisig's salt nonce).
    Random { bytes: u32 },
    /// `owner` (a Vela account on this device) signs `challenge` (hex) with
    /// its passkey — in Vela, or on the trusted page it signs on.
    Passkey {
        owner: String,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        challenge: String,
        purpose: PasskeyPurpose,
    },
    /// Ask the person whether to sign on a network where more than the
    /// members can act for the multisig (`RiskConfirmSheet`): every
    /// signature asks, never remembered. A shell that cannot ask answers
    /// `risk_answered { signs: false }`.
    AskRisk {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    /// `from` (a Vela account on this device) sends `calls` on `chain_id`
    /// as one user operation, through the send pipeline the shell already
    /// has (its fee quote, its passkey, its relay). `fee` is the quote the
    /// person saw, when there was one.
    SendCalls {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        from: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        calls: Vec<SentCall>,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        fee: Option<Value>,
        /// What it is for ([`SendPurpose`]); absent from an older machine.
        #[serde(default)]
        purpose: Option<SendPurpose>,
    },
    /// Quote what `from` sending `calls` costs and whether it can pay
    /// (`preflight`): the shell's fee pipeline, nothing sent.
    QuoteCalls {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        from: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        calls: Vec<SentCall>,
    },
    /// Open a PeerConnect room (an outside owner's wallet page and Vela meet
    /// there; Noise end to end, the relay sees nothing). `invite`: join the
    /// one a link carries; `None`: create one and answer its invite.
    PeerOpen {
        relay: Option<String>,
        invite: Option<String>,
        /// A pairing kept from an earlier session, to reconnect without a scan.
        pairing: Option<String>,
    },
    /// Send one message on an open room.
    PeerSend {
        room: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        message: Value,
    },
    /// The next message on an open room (answered when one arrives, or the
    /// room closes).
    PeerReceive { room: String },
    /// Close a room (`walletpair_cancel` is sent first when `cancel`).
    /// `end`: the pairing ends for both sides (the room is closed and its
    /// record forgotten); otherwise this side only leaves it, and the
    /// pairing reopens it next time.
    PeerClose {
        room: String,
        cancel: bool,
        #[serde(default)]
        end: bool,
    },
    /// One EIP-1193 request to the wallet injected in this page (the
    /// connect page acting for an outside owner).
    WalletRequest {
        method: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        params: Value,
    },
    /// Simulate `calls` from `from` (the Safe) on `chain_id` — the signing
    /// sheet's own simulation (`rpcSimulateReply`, `eth_simulateV1`), as
    /// the multisig review reads it. Answered `simulated`, with `None`
    /// when the pool gave up (the review says "could not check").
    Simulate {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        from: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        calls: Vec<crate::multisig_review::SimCall>,
    },
    /// What the send flow knows of a recipient (`resolveRecipientRisk`):
    /// whether it is a contract, and whether this device sent to it before.
    RecipientRisk {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        address: String,
    },
    /// Where a user operation stands with the relay (`pollUserOpStatus`):
    /// answered `user_op_status`.
    UserOpStatus {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        user_op_hash: String,
    },
    /// Something the shell routes on: another machine to tell, rows to
    /// write, a screen to refresh. Answered `ack`.
    Notice { notice: Notice },
}

/// What a machine tells the shell to route (one machine's change another
/// must hear of — the web's single store, split across cores).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigNotice"))]
pub enum Notice {
    /// A multisig on this device changed: the accounts machine applies it
    /// (`multisig_accounts::Event::Apply`).
    Account {
        address: String,
        update: AccountUpdate,
    },
    /// Rows for the activity (executed transactions), in the shape the
    /// shell's own transaction store keeps (`LocalTransaction`).
    ActivityRows {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        rows: Vec<Value>,
    },
    /// The proposal mailbox took something for `safe`: whoever shows it
    /// reads it again.
    MailboxTook { safe: String },
    /// The multisig shown is this one now (`None`: the Vela account).
    Activated { address: Option<String> },
    /// A proposal link opened (`multisig_accounts::Event::OpenLink`): the
    /// proposals machine opens that proposal's sheet
    /// (`multisig_proposals::Event::OpenLinked` with these fields). `preview`:
    /// no member of it is on this device — the multisig as read, kept
    /// nowhere and not shown, its transaction read only (ms-21); `None`: it
    /// is `address`, on this device and activated.
    OpenProposal {
        address: String,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        preview: Option<Box<MultisigAccount>>,
    },
    /// No member is on this device: an outside member's wallet signs this
    /// SafeTx over a live session — the shell starts the session machine
    /// (`multisig_outside::session::Event::Start` with this `request`) and
    /// hands its outcome back (`multisig_proposals::Event::OutsideAnswered`).
    OutsideSign {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        request: Box<super::multisig_outside::session::SessionRequest>,
    },
}

/// One change to a stored multisig, made by a machine that does not own
/// the list (the web's `multisig.useChain`, `setFindingsOn`, …).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAccountUpdate"))]
pub enum AccountUpdate {
    /// It is used on `chain_id` (its proposals are looked for there).
    UseChain {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
    },
    /// What the chain says now about `chain_id` replaces that network's findings.
    FindingsOn {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    /// The name its record holds now.
    RecordedName { name: String },
    /// What this device knows of its members (`typed`: the person's own word).
    NameMembers {
        members: BTreeMap<String, MemberInfo>,
        typed: bool,
    },
    /// r8-7: its members as the chains say now — its members where it is
    /// deployed, its founding ones where it is not (`members_anywhere`).
    MembersNow { members: Vec<String> },
    /// r9-14: the rule in force as the chains last said — its rule where it
    /// is deployed, the founding one where it is not, per network
    /// (`current_rules`) — kept per multisig and served on its row.
    RuleNow {
        #[cfg_attr(feature = "bindings", ts(type = "MultisigRuleChip"))]
        chip: super::multisig_members::RuleChip,
    },
}

/// What the shell answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigShellResult"))]
pub enum MultisigShellResult {
    /// It answered: the status, its `Retry-After` header when it could be
    /// read, and the body as text.
    HttpAnswered {
        status: u16,
        retry_after: Option<String>,
        body: String,
    },
    /// Nothing answered (refused, closed, offline, timed out).
    HttpUnreachable {
        detail: String,
    },
    RpcResult {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        result: Value,
    },
    /// The node answered with a JSON-RPC error: its code, message and
    /// `data` as the node gave it (r8-4: a revert's payload — the
    /// state-diff probe reads its answer there). `data` absent: none.
    RpcError {
        code: Option<i64>,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "bindings", ts(optional, type = "unknown"))]
        data: Option<Value>,
    },
    /// No node answered (or not within `bound_ms`).
    RpcUnanswered {
        detail: String,
    },
    StoreLoaded {
        entries: BTreeMap<String, Option<String>>,
    },
    StoreWritten,
    StoreFailed {
        detail: String,
    },
    Waited,
    Now {
        now_ms: f64,
    },
    Random {
        hex: String,
    },
    /// The passkey signed. Hex, as `navigator.credentials.get` gives it.
    Asserted {
        credential_id: String,
        authenticator_data: String,
        client_data_json: String,
        signature: String,
    },
    /// The member's trusted signing page approved (it reviewed and signed
    /// there): the approval it answered, checked again here.
    PageApproved {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        approval: Value,
    },
    /// The person closed the prompt (or the page): a cancel, not a failure.
    PasskeyCancelled,
    /// The passkey could not sign (a key nothing here reaches, a venue
    /// that blocks it): said as `detail`.
    PasskeyFailed {
        detail: String,
    },
    RiskAnswered {
        signs: bool,
    },
    /// The simulation's reply (the signing sheet's envelope), or `None`.
    Simulated {
        reply: Option<String>,
    },
    /// The relay's word on a user operation: `status` is one of
    /// `pending`, `included`, `failed`, `rejected`, `not_found`; `None`:
    /// it did not answer.
    UserOpStatus {
        status: Option<String>,
        tx_hash: Option<String>,
    },
    /// A recipient's facts; `None`: unread.
    RecipientFacts {
        is_contract: Option<bool>,
        first_time: Option<bool>,
    },
    /// The user operation is with the relay.
    CallsSent {
        user_op_hash: String,
        tx_hash: Option<String>,
    },
    /// It was not sent. `maybe_sent`: it may have reached the relay (a
    /// lost answer) — followed, never sent again.
    CallsFailed {
        detail: String,
        maybe_sent: bool,
    },
    CallsCancelled,
    /// The fee for `quote_calls`, as the shell's fee pipeline reads it.
    Quoted {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        quote: Value,
    },
    QuoteFailed {
        detail: String,
    },
    PeerOpened {
        room: String,
        invite: Option<String>,
        pairing: Option<String>,
    },
    PeerMessage {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        message: Value,
    },
    PeerClosed {
        reason: String,
    },
    WalletAnswered {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        result: Value,
    },
    WalletRefused {
        code: Option<i64>,
        message: String,
    },
    Ack,
}

impl Operation for MultisigOperation {
    type Output = MultisigShellResult;
}

/// The effect every multisig machine emits.
#[effect]
pub enum MultisigEffect {
    Render(RenderOperation),
    Shell(MultisigOperation),
}

impl super::SplitEffect for MultisigEffect {
    type Op = MultisigOperation;
    fn into_shell(self) -> Option<crux_core::Request<MultisigOperation>> {
        match self {
            MultisigEffect::Render(_) => None,
            MultisigEffect::Shell(request) => Some(request),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared state and the async helper
// ---------------------------------------------------------------------------

/// State a machine's async commands share with its `update` — a lock held
/// only between awaits (a guard is not `Send`, so the compiler refuses one
/// held across a shell request).
pub type Shared<T> = Arc<Mutex<T>>;

/// Take the lock; a panic elsewhere left the value as it was, never half-
/// written by this module (every mutation is one assignment), so it is used.
pub fn lock<T>(shared: &Shared<T>) -> MutexGuard<'_, T> {
    shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What a read through the RPC pool came to when it did not answer a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcFail {
    /// The node answered an error (its `data`, if any, kept: r8-4).
    Error {
        code: Option<i64>,
        message: String,
        data: Option<Value>,
    },
    /// Nothing answered.
    Unanswered(String),
}

impl RpcFail {
    pub fn message(&self) -> String {
        match self {
            Self::Error { message, .. } | Self::Unanswered(message) => message.clone(),
        }
    }
}

/// An HTTP answer, or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpAnswer {
    Answered {
        status: u16,
        retry_after: Option<String>,
        body: String,
    },
    Unreachable(String),
}

/// The shell, as an async command sees it. Cheap to clone.
pub struct Io<Ev: Send + 'static> {
    ctx: CommandContext<MultisigEffect, Ev>,
}

impl<Ev: Send + 'static> Clone for Io<Ev> {
    fn clone(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
        }
    }
}

impl<Ev: Send + 'static> Io<Ev> {
    pub fn new(ctx: CommandContext<MultisigEffect, Ev>) -> Self {
        Self { ctx }
    }

    /// Ask the shell, and wait for its answer.
    pub async fn ask(&self, op: MultisigOperation) -> MultisigShellResult {
        self.ctx.request_from_shell(op).await
    }

    /// Run `f` alongside, in the same command (a resend that goes on after
    /// the person was told it is kept).
    pub fn spawn<F, Fut>(&self, f: F)
    where
        F: FnOnce(Io<Ev>) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let io = self.clone();
        // Dropping the handle detaches the task; it runs to its end.
        drop(self.ctx.spawn(move |_| f(io)));
    }

    /// Hand an event back to the machine's `update`.
    pub fn emit(&self, event: Ev) {
        self.ctx.send_event(event);
    }

    pub async fn now(&self) -> f64 {
        match self.ask(MultisigOperation::Now).await {
            MultisigShellResult::Now { now_ms } => now_ms,
            _ => 0.0,
        }
    }

    pub async fn wait(&self, ms: u64) {
        let ms = u32::try_from(ms).unwrap_or(u32::MAX);
        let _ = self.ask(MultisigOperation::Wait { ms }).await;
    }

    pub async fn random_hex(&self, bytes: u32) -> Option<String> {
        match self.ask(MultisigOperation::Random { bytes }).await {
            MultisigShellResult::Random { hex } => Some(hex),
            _ => None,
        }
    }

    pub async fn http(
        &self,
        method: &str,
        url: String,
        body: Option<String>,
        timeout_ms: u32,
    ) -> HttpAnswer {
        match self
            .ask(MultisigOperation::Http {
                method: method.to_owned(),
                url,
                body,
                timeout_ms,
            })
            .await
        {
            MultisigShellResult::HttpAnswered {
                status,
                retry_after,
                body,
            } => HttpAnswer::Answered {
                status,
                retry_after,
                body,
            },
            MultisigShellResult::HttpUnreachable { detail } => HttpAnswer::Unreachable(detail),
            other => HttpAnswer::Unreachable(format!("unexpected answer {other:?}")),
        }
    }

    /// One call through the pool: its result, or why there is none.
    pub async fn rpc(&self, chain_id: u64, method: &str, params: Value) -> Result<Value, RpcFail> {
        self.rpc_within(chain_id, method, params, None).await
    }

    /// As [`Io::rpc`], waited for at most `bound_ms`.
    pub async fn rpc_within(
        &self,
        chain_id: u64,
        method: &str,
        params: Value,
        bound_ms: Option<u32>,
    ) -> Result<Value, RpcFail> {
        match self
            .ask(MultisigOperation::Rpc {
                chain_id,
                method: method.to_owned(),
                params,
                bound_ms,
            })
            .await
        {
            MultisigShellResult::RpcResult { result } => Ok(result),
            MultisigShellResult::RpcError {
                code,
                message,
                data,
            } => Err(RpcFail::Error {
                code,
                message,
                data,
            }),
            MultisigShellResult::RpcUnanswered { detail } => Err(RpcFail::Unanswered(detail)),
            other => Err(RpcFail::Unanswered(format!("unexpected answer {other:?}"))),
        }
    }

    /// `eth_call` at `latest`: its return data.
    pub async fn eth_call(&self, chain_id: u64, to: &str, data: &str) -> Result<String, RpcFail> {
        let result = self
            .rpc(
                chain_id,
                "eth_call",
                json!([{ "to": to, "data": data }, "latest"]),
            )
            .await?;
        result
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| RpcFail::Unanswered("eth_call answered no data".to_owned()))
    }

    /// Write one store key; a store that fails is best effort, as the web's.
    pub async fn store(&self, key: &str, value: Option<String>) {
        let _ = self
            .ask(MultisigOperation::WriteStore {
                key: key.to_owned(),
                value,
            })
            .await;
    }

    /// Read store keys: each key's value, or `None`.
    pub async fn load(&self, keys: &[&str]) -> BTreeMap<String, Option<String>> {
        match self
            .ask(MultisigOperation::LoadStore {
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
            })
            .await
        {
            MultisigShellResult::StoreLoaded { entries } => entries,
            _ => BTreeMap::new(),
        }
    }

    pub async fn notice(&self, notice: Notice) {
        let _ = self.ask(MultisigOperation::Notice { notice }).await;
    }
}

// ---------------------------------------------------------------------------
// Records the machines share
// ---------------------------------------------------------------------------

/// The store keys, exactly as the web writes them — a native shell's store
/// holds the same records under the same names.
pub mod keys {
    pub const ACCOUNTS: &str = "vela.multisigAccounts";
    pub const ACTIVE: &str = "vela.activeMultisig";
    pub const PENDING_PROOFS: &str = "vela.multisigPendingProofs";
    pub const DISCOVERED: &str = "vela.multisigDiscovered";
    pub const DEPLOYED: &str = "vela.multisigDeployed";
    pub const OUTBOX: &str = "vela.multisigOutbox";
    pub const WAITING: &str = "vela.multisigWaiting";
    pub const EXECUTIONS: &str = "vela.multisigExecutions";
    pub const LAST_ACTOR: &str = "vela.multisigLastActor";
}

/// A member's label and kind, as one device knows it (never authority).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberInfo"))]
pub struct MemberInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `passkey` (a Vela account) or `wallet` (an outside wallet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The person typed `name` on this device (r5reg-16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed: Option<bool>,
}

/// A multisig this device added — `store.svelte.ts`' `MultisigAccount`,
/// field for field (the stored JSON is the same).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAccountRecord"))]
pub struct MultisigAccount {
    pub address: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub genesis: Genesis,
    /// The founding members (the genesis's), in their order.
    pub owners: Vec<String>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    pub recorded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<bool>,
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub chain_ids: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<BTreeMap<String, MemberInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_run: Option<bool>,
    #[serde(default, serialize_with = "int_ms")]
    pub added_ms: f64,
    /// `Finding`s, as the web stores them: `{from: 'genesis', chainId, reason}`
    /// or `{from: 'chain', chainId, risk}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(type = "unknown[] | null"))]
    pub findings: Option<Vec<Value>>,
}

impl MultisigAccount {
    /// Whether another device can find it by its address (`isShared`).
    pub fn is_shared(&self) -> bool {
        self.shared == Some(true) || self.recorded
    }

    /// Its findings on `chain_id`.
    pub fn findings_on(&self, chain_id: u64) -> Vec<Value> {
        self.findings
            .iter()
            .flatten()
            .filter(|f| finding_chain(f) == Some(chain_id))
            .cloned()
            .collect()
    }
}

/// The chain a stored finding is about.
pub fn finding_chain(finding: &Value) -> Option<u64> {
    finding.get("chainId").and_then(Value::as_u64)
}

/// How a Vela account approves for a multisig (`multisigApprovalRoute`):
/// in Vela, on the trusted page it signs on, or not at all from here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigApprovalRoute"))]
pub enum ApprovalRoute {
    #[default]
    Vela,
    Page,
    Blocked,
}

/// A founding key of a Vela account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigOwnerKey"))]
pub struct OwnerKey {
    pub credential_id: String,
    pub public_key_hex: String,
}

/// A Vela account on this device (what the session holds), as the multisig
/// machines need it: its address, name, keys (for `approval_from_assertion`
/// and the owner proof) and how it approves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigLocalAccount"))]
pub struct LocalAccount {
    pub address: String,
    pub name: String,
    pub keys: Vec<OwnerKey>,
    #[serde(default)]
    pub route: ApprovalRoute,
}

impl LocalAccount {
    pub fn wallet_keys(&self) -> Vec<crate::user_op::WalletKey> {
        self.keys
            .iter()
            .map(|k| crate::user_op::WalletKey {
                credential_id: k.credential_id.clone(),
                public_key_hex: k.public_key_hex.clone(),
            })
            .collect()
    }
}

/// This device's Vela accounts among `owners`, in the owners' order, each
/// once (`localOwners`).
pub fn local_owners(accounts: &[LocalAccount], owners: &[String]) -> Vec<LocalAccount> {
    let mut out: Vec<LocalAccount> = Vec::new();
    for owner in owners {
        if let Some(account) = accounts.iter().find(|a| same_address(&a.address, owner)) {
            if !out.iter().any(|o| same_address(&o.address, owner)) {
                out.push(account.clone());
            }
        }
    }
    out
}

/// Two addresses, whatever their case (`sameAddress`); `None` on either side is not the same.
pub fn same_address(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// EIP-55, or the address as given when it is not one.
pub fn checksummed(address: &str) -> String {
    crate::primitives::checksum_address(address).unwrap_or_else(|_| address.to_owned())
}

// ---------------------------------------------------------------------------
// The service's outage (`service-state.svelte.ts`)
// ---------------------------------------------------------------------------

/// How long background reads wait after the service was found away, by how
/// many probes in a row found it so.
pub const RETRY_STEPS_MS: [f64; 5] = [15_000.0, 30_000.0, 60_000.0, 120_000.0, 300_000.0];

/// A millisecond time as the web writes it: `Date.now()` is an integer, so
/// a whole value is written without a fraction.
pub fn int_ms<S: serde::Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        #[allow(clippy::cast_possible_truncation)]
        return s.serialize_i64(*v as i64);
    }
    s.serialize_f64(*v)
}

/// Whether the service is answering: the first request that finds nothing
/// answering marks it away, and background reads then ask nothing until the
/// next step of a capped schedule, when one probe goes first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outage {
    pub down: bool,
    failures: usize,
    retry_at: f64,
    busy_until: f64,
}

impl Outage {
    /// Away and its next step has not come — or busy: background reads ask nothing.
    pub fn blocked(&self, now: f64) -> bool {
        (self.down && now < self.retry_at) || now < self.busy_until
    }

    pub fn busy(&mut self, until: f64) {
        self.busy_until = self.busy_until.max(until);
    }

    pub fn retry_at(&self) -> f64 {
        if self.down {
            self.retry_at
        } else {
            0.0
        }
    }

    pub fn failed(&mut self, now: f64) {
        if self.down && now < self.retry_at {
            return;
        }
        self.failures += 1;
        self.down = true;
        let step = RETRY_STEPS_MS[self.failures.min(RETRY_STEPS_MS.len()) - 1];
        self.retry_at = now + step;
    }

    pub fn answered(&mut self) {
        if self.down || self.failures > 0 {
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Before a round of background reads: `Some(answer)` when it is decided
    /// without a probe, `None` when one probe should go first.
    pub fn available_now(&self, now: f64) -> Option<bool> {
        if now < self.busy_until {
            return Some(false);
        }
        if !self.down {
            return Some(true);
        }
        if self.blocked(now) {
            return Some(false);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// The genesis service and its mailbox (`service.ts`)
// ---------------------------------------------------------------------------

/// The default multisig service (`DEFAULT_SERVICE_ENDPOINTS.genesisServiceURL`).
pub const DEFAULT_GENESIS_SERVICE_URL: &str = "https://safe-genesis.getvela.app";

const TIMEOUT_MS: u32 = 15_000;
const ATTEMPTS: u32 = 3;
/// The longest `429` wait waited out before asking once more.
pub const RETRY_AFTER_MAX_MS: f64 = 10_000.0;
const CHECK_TIMEOUT_MS: u32 = 8_000;
const SERVED_TTL_MS: f64 = 60_000.0;
const ALL_PAGE: usize = 100;
const ALL_PAGES: usize = 10;

/// Why the service gave no usable answer.
#[derive(Clone, Debug, PartialEq)]
pub enum ServiceFail {
    /// It could not be reached, or answered something unreadable.
    Unavailable(String),
    /// A write it is over its limit for (`429`): never a failure — sent
    /// again after the wait it named (`None`: it named none).
    Busy(Option<f64>),
}

/// How long a `429` says to wait, in ms: its `Retry-After` seconds, else
/// the seconds its body names ("retry in 3 s"). `retryAfterMs`.
pub fn retry_after_ms(header: Option<&str>, body: &str) -> Option<f64> {
    let header = header.map(str::trim).unwrap_or("");
    let seconds = if !header.is_empty() && header.bytes().all(|b| b.is_ascii_digit()) {
        Some(header.to_owned())
    } else {
        retry_in(body)
    };
    seconds
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| s * 1000.0)
}

/// `/retry in (\d+) ?s\b/`.
fn retry_in(body: &str) -> Option<String> {
    let mut rest = body;
    while let Some(at) = rest.find("retry in ") {
        let after = &rest[at + "retry in ".len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            let tail = &after[digits.len()..];
            let tail = tail.strip_prefix(' ').unwrap_or(tail);
            if let Some(after_s) = tail.strip_prefix('s') {
                let boundary = after_s
                    .chars()
                    .next()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                if boundary {
                    return Some(digits);
                }
            }
        }
        rest = after;
    }
    None
}

/// What the service says of itself (`/api/health`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceTable {
    pub chains: Option<BTreeSet<u64>>,
    pub registry_chains: Option<BTreeSet<u64>>,
    pub home_chain_id: Option<u64>,
}

/// One registry's entry for a Safe (`RegistryEntry`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegistryEntry {
    pub chain_id: u64,
    pub name: Option<String>,
    pub name_by_safe: bool,
    pub renamed_at: u64,
    pub index: u64,
    pub recorder: String,
    pub recorded_at: u64,
}

/// `GET /api/genesis/{safe}` (`GenesisAnswer`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GenesisAnswer {
    pub found: bool,
    pub pending: Option<bool>,
    pub genesis: Option<Genesis>,
    pub recorded_on: Option<Vec<u64>>,
    pub read_from: Option<u64>,
    pub verified: Option<bool>,
    pub name_by_safe: Option<bool>,
    pub renamed_at: Option<u64>,
    pub registries: Option<Vec<RegistryEntry>>,
}

/// An owner's passkey asking for a multisig deployed nowhere yet to be recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerProof {
    pub owner: String,
    pub owner_genesis: Genesis,
    pub authenticator_data: String,
    /// The exact text the authenticator signed.
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String,
    pub signature: String,
}

/// `POST /api/import`'s answer (`ImportAnswer`).
#[derive(Clone, Debug, PartialEq)]
pub enum ImportAnswer {
    Recorded { recorded_on: Vec<u64> },
    Queued { task_id: String },
    Resolved { reason: String, record_call: Value },
    Refused { refusal: Value },
    Error { error: String },
}

/// A stored approval: an [`multisig::Approval`] with when the mailbox took it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredApproval {
    #[serde(flatten)]
    pub approval: multisig::Approval,
    #[serde(default, serialize_with = "int_ms")]
    pub created_ms: f64,
}

/// One proposal the mailbox holds (`Proposal`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigMailboxProposal")
)]
pub struct Proposal {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe: String,
    pub safe_tx_hash: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub safe_tx: SafeTx,
    #[serde(default)]
    pub proposer: String,
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
    pub approvals: Vec<StoredApproval>,
    #[serde(default, serialize_with = "int_ms")]
    pub created_ms: f64,
    #[serde(default, serialize_with = "int_ms")]
    pub updated_ms: f64,
}

/// Who the MAILBOX judges approvals by — never what this wallet counts by.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MailboxMembers {
    pub deployed: bool,
    pub owners: Vec<String>,
    pub threshold: String,
    pub nonce: String,
}

/// `GET /api/proposals/{safe}?chainId=` (`ProposalsAnswer`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProposalsAnswer {
    pub members: MailboxMembers,
    pub proposals: Vec<Proposal>,
}

/// `GET /api/proposals/{safe}` across chains (`AllProposalsAnswer`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllProposalsAnswer {
    pub proposals: Vec<Proposal>,
    pub chains: Vec<(u64, u64)>,
}

/// One owner's approval to hand to the mailbox (`submitApproval`'s request).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    pub chain_id: u64,
    pub safe: String,
    pub safe_tx: SafeTx,
    pub approval: multisig::Approval,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis: Option<Genesis>,
}

/// `POST /api/proposals`' answer (`SubmitAnswer`).
#[derive(Clone, Debug, PartialEq)]
pub enum SubmitAnswer {
    Took {
        status: String,
        proposal: Option<Box<Proposal>>,
    },
    Refused {
        refusal: Value,
    },
}

/// The service in use, with what this session knows of it. Cheap to clone:
/// every field is shared, so a command reads and updates the one copy.
#[derive(Clone, Debug, Default)]
pub struct Service {
    pub url: Shared<String>,
    pub outage: Shared<Outage>,
    /// Services found without the cross-chain list this session.
    without_all: Shared<BTreeSet<String>>,
    served: Shared<Option<(String, f64, Option<ServiceTable>)>>,
}

impl Service {
    pub fn new(url: &str) -> Self {
        let service = Self::default();
        *lock(&service.url) = url.to_owned();
        service
    }

    pub fn url(&self) -> String {
        let url = lock(&self.url).clone();
        if url.is_empty() {
            DEFAULT_GENESIS_SERVICE_URL.to_owned()
        } else {
            url
        }
    }

    /// Another service is in use (Settings): what was known of the last ends.
    pub fn set_url(&self, url: &str) {
        let mut current = lock(&self.url);
        if *current != url {
            lock(&self.outage).reset();
            *lock(&self.served) = None;
        }
        *current = url.to_owned();
    }

    /// One request, with the service's retry rules (`service.ts` `request`).
    /// `Ok((status, body))`; a body that is not JSON is retried as unreadable.
    pub async fn request<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        method: &str,
        path: &str,
        body: Option<String>,
        background: bool,
        write: bool,
    ) -> Result<(u16, Value), ServiceFail> {
        let mut last = "no attempt".to_owned();
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                io.wait(400 * 2u64.pow(attempt)).await;
            }
            let now = if background { io.now().await } else { 0.0 };
            if background && lock(&self.outage).blocked(now) {
                return Err(ServiceFail::Unavailable(format!("away: {last}")));
            }
            let answer = io
                .http(
                    method,
                    format!("{}{path}", self.url()),
                    body.clone(),
                    TIMEOUT_MS,
                )
                .await;
            let (status, retry_after, text) = match answer {
                HttpAnswer::Unreachable(detail) => {
                    let now = io.now().await;
                    lock(&self.outage).failed(now);
                    last = detail;
                    continue;
                }
                HttpAnswer::Answered {
                    status,
                    retry_after,
                    body,
                } => (status, retry_after, body),
            };
            lock(&self.outage).answered();
            if status >= 500 && attempt < ATTEMPTS - 1 {
                last = format!("HTTP {status}");
                continue;
            }
            let wait = if status == 429 {
                retry_after_ms(retry_after.as_deref(), &text)
            } else {
                None
            };
            if status == 429 {
                if write {
                    return Err(ServiceFail::Busy(wait));
                }
                let now = io.now().await;
                lock(&self.outage).busy(now + wait.unwrap_or(RETRY_AFTER_MAX_MS));
            }
            if let Some(ms) = wait {
                if ms <= RETRY_AFTER_MAX_MS && attempt < ATTEMPTS - 1 {
                    last = "HTTP 429".to_owned();
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    io.wait(ms as u64).await;
                    continue;
                }
            }
            match serde_json::from_str::<Value>(&text) {
                Ok(json) => return Ok((status, json)),
                Err(error) => last = error.to_string(),
            }
        }
        Err(ServiceFail::Unavailable(last))
    }

    /// What answers at `url` (`GET /api/health`): one try, no retries.
    pub async fn check<Ev: Send + 'static>(
        io: &Io<Ev>,
        url: &str,
    ) -> super::genesis_service::ServiceAnswer {
        match io
            .http("GET", format!("{url}/api/health"), None, CHECK_TIMEOUT_MS)
            .await
        {
            HttpAnswer::Answered { status, body, .. } => {
                super::genesis_service::service_answer(status, &body)
            }
            HttpAnswer::Unreachable(_) => super::genesis_service::ServiceAnswer::Unreachable,
        }
    }

    /// Whether the service in use answers as itself now (the outage's probe).
    pub async fn probe<Ev: Send + 'static>(&self, io: &Io<Ev>) -> bool {
        matches!(
            Self::check(io, &self.url()).await,
            super::genesis_service::ServiceAnswer::Genesis { .. }
        )
    }

    /// For a round of background reads: whether to make them
    /// (`serviceOutage.available(probeService)`).
    pub async fn available<Ev: Send + 'static>(&self, io: &Io<Ev>) -> bool {
        let now = io.now().await;
        let decided = lock(&self.outage).available_now(now);
        if let Some(answer) = decided {
            return answer;
        }
        let up = self.probe(io).await;
        let now = io.now().await;
        let mut outage = lock(&self.outage);
        if up {
            outage.answered();
        } else {
            outage.failed(now);
        }
        up
    }

    /// The service's own table, kept a minute (`serviceTable`).
    pub async fn table<Ev: Send + 'static>(&self, io: &Io<Ev>) -> Option<ServiceTable> {
        let url = self.url();
        let now = io.now().await;
        if let Some((at_url, at, table)) = lock(&self.served).clone() {
            if at_url == url && now - at < SERVED_TTL_MS {
                return table;
            }
        }
        let table = match Self::check(io, &url).await {
            super::genesis_service::ServiceAnswer::Genesis {
                chains,
                home_chain_id,
                ..
            } => Some(ServiceTable {
                chains: chains
                    .as_ref()
                    .map(|c| c.iter().map(|c| c.chain_id).collect()),
                registry_chains: chains.as_ref().map(|c| {
                    c.iter()
                        .filter(|c| c.registry)
                        .map(|c| c.chain_id)
                        .collect()
                }),
                home_chain_id,
            }),
            _ => None,
        };
        if table.is_some() {
            *lock(&self.served) = Some((url, now, table.clone()));
        }
        table
    }

    /// The chains the service reads, or `None` when that is not known.
    pub async fn served_chains<Ev: Send + 'static>(&self, io: &Io<Ev>) -> Option<BTreeSet<u64>> {
        self.table(io).await.and_then(|t| t.chains)
    }

    /// `GET /api/genesis/{safe}`.
    pub async fn fetch_genesis<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        safe: &str,
        background: bool,
    ) -> Result<GenesisAnswer, ServiceFail> {
        let (status, body) = self
            .request(
                io,
                "GET",
                &format!("/api/genesis/{safe}"),
                None,
                background,
                false,
            )
            .await?;
        if status == 404 {
            return Ok(GenesisAnswer::default());
        }
        if status != 200 {
            return Err(ServiceFail::Unavailable(format!("HTTP {status}")));
        }
        serde_json::from_value(body).map_err(|e| ServiceFail::Unavailable(e.to_string()))
    }

    /// `POST /api/import` (idempotent).
    pub async fn import_genesis<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        req: Value,
    ) -> Result<ImportAnswer, ServiceFail> {
        let (status, body) = self
            .request(
                io,
                "POST",
                "/api/import",
                Some(req.to_string()),
                false,
                true,
            )
            .await?;
        if status == 422 {
            return Ok(ImportAnswer::Refused {
                refusal: body.get("refusal").cloned().unwrap_or(Value::Null),
            });
        }
        if status >= 400 {
            return Ok(ImportAnswer::Error {
                error: body
                    .get("error")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("HTTP {status}"), str::to_owned),
            });
        }
        Ok(match body.get("status").and_then(Value::as_str) {
            Some("recorded") => ImportAnswer::Recorded {
                recorded_on: body
                    .get("recordedOn")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default(),
            },
            Some("queued") => ImportAnswer::Queued {
                task_id: body
                    .get("taskId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            },
            Some("resolved") => ImportAnswer::Resolved {
                reason: body
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                record_call: body.get("recordCall").cloned().unwrap_or(Value::Null),
            },
            Some("refused") => ImportAnswer::Refused {
                refusal: body.get("refusal").cloned().unwrap_or(Value::Null),
            },
            _ => ImportAnswer::Error {
                error: body
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown answer")
                    .to_owned(),
            },
        })
    }

    /// `GET /api/task/{id}`: its status, or `None`.
    pub async fn fetch_task<Ev: Send + 'static>(&self, io: &Io<Ev>, id: &str) -> Option<String> {
        let (status, body) = self
            .request(io, "GET", &format!("/api/task/{id}"), None, false, false)
            .await
            .ok()?;
        (status == 200)
            .then(|| {
                body.get("status")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .flatten()
    }

    /// A multisig's proposals on one chain; `Ok(None)`: the service knows nothing of it.
    pub async fn fetch_proposals<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        chain_id: u64,
        safe: &str,
        background: bool,
    ) -> Result<Option<ProposalsAnswer>, ServiceFail> {
        let (status, body) = self
            .request(
                io,
                "GET",
                &format!("/api/proposals/{safe}?chainId={chain_id}"),
                None,
                background,
                false,
            )
            .await?;
        if status == 404 {
            return Ok(None);
        }
        if status != 200 {
            return Err(ServiceFail::Unavailable(format!("HTTP {status}")));
        }
        serde_json::from_value(body)
            .map(Some)
            .map_err(|e| ServiceFail::Unavailable(e.to_string()))
    }

    /// Every proposal the mailbox holds for `safe` on every chain, paged;
    /// `Ok(None)`: this service has no such list (asked again only when the
    /// service changes).
    pub async fn fetch_all_proposals<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        safe: &str,
        background: bool,
    ) -> Result<Option<AllProposalsAnswer>, ServiceFail> {
        let url = self.url();
        if lock(&self.without_all).contains(&url) {
            return Ok(None);
        }
        let mut proposals: Vec<Proposal> = Vec::new();
        let mut chains = Vec::new();
        for page in 0..ALL_PAGES {
            let offset = page * ALL_PAGE;
            let (status, body) = self
                .request(
                    io,
                    "GET",
                    &format!("/api/proposals/{safe}?offset={offset}&limit={ALL_PAGE}"),
                    None,
                    background,
                    false,
                )
                .await?;
            if status == 404 || status == 400 {
                lock(&self.without_all).insert(url);
                return Ok(None);
            }
            let Some(list) = body.get("proposals").and_then(Value::as_array) else {
                return Err(ServiceFail::Unavailable(format!("HTTP {status}")));
            };
            if status != 200 {
                return Err(ServiceFail::Unavailable(format!("HTTP {status}")));
            }
            let read: Vec<Proposal> = list
                .iter()
                .filter_map(|p| serde_json::from_value(p.clone()).ok())
                .collect();
            let empty = list.is_empty();
            proposals.extend(read);
            if page == 0 {
                chains = body
                    .get("chains")
                    .and_then(Value::as_array)
                    .map(|c| {
                        c.iter()
                            .filter_map(|c| {
                                Some((c.get("chainId")?.as_u64()?, c.get("count")?.as_u64()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            let total = body
                .get("total")
                .and_then(Value::as_u64)
                .map_or(proposals.len(), |t| {
                    usize::try_from(t).unwrap_or(usize::MAX)
                });
            if empty || proposals.len() >= total {
                break;
            }
        }
        Ok(Some(AllProposalsAnswer { proposals, chains }))
    }

    /// Put a transaction and one owner's approval in the mailbox (idempotent).
    pub async fn submit_approval<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        req: &Submission,
    ) -> Result<SubmitAnswer, ServiceFail> {
        let body =
            serde_json::to_string(req).map_err(|e| ServiceFail::Unavailable(e.to_string()))?;
        let (status, body) = self
            .request(io, "POST", "/api/proposals", Some(body), false, true)
            .await?;
        if status == 200 || status == 201 {
            return Ok(SubmitAnswer::Took {
                status: body
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("unchanged")
                    .to_owned(),
                proposal: body
                    .get("proposal")
                    .and_then(|p| serde_json::from_value(p.clone()).ok()),
            });
        }
        if status == 422 {
            return Ok(SubmitAnswer::Refused {
                refusal: body.get("refusal").cloned().unwrap_or(Value::Null),
            });
        }
        if (400..500).contains(&status) {
            return Ok(SubmitAnswer::Refused {
                refusal: json!({
                    "kind": "error",
                    "status": status,
                    "error": body.get("error").cloned().unwrap_or(Value::Null),
                }),
            });
        }
        Err(ServiceFail::Unavailable(
            body.get("error")
                .and_then(Value::as_str)
                .map_or_else(|| format!("HTTP {status}"), str::to_owned),
        ))
    }
}

// ---------------------------------------------------------------------------
// The chain, read through the wallet's own RPC (`chain.ts`)
// ---------------------------------------------------------------------------

const GET_OWNERS: &str = "0xa0e67e2b";
const GET_THRESHOLD: &str = "0xe75235b8";
const NONCE: &str = "0xaffed0e0";
/// Members read again a minute on regardless of the nonce (a module can
/// change them without a transaction).
pub const MEMBERS_FRESH_MS: f64 = 60_000.0;
const DEPLOYED_CAP: usize = 200;

/// A multisig on one chain right now (`ChainMembers`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigChainMembers"))]
pub struct ChainMembers {
    pub deployed: bool,
    pub owners: Vec<String>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    /// Decimal (the web's `bigint`).
    #[serde(with = "dec_u128")]
    #[cfg_attr(feature = "bindings", ts(type = "string"))]
    pub nonce: u128,
}

/// A nonce as a decimal string on the wire (a `bigint` the web prints).
pub mod dec_u128 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            S(String),
            N(u64),
        }
        match Either::deserialize(d)? {
            Either::S(s) => super::quantity(&s).ok_or_else(|| serde::de::Error::custom("nonce")),
            Either::N(n) => Ok(u128::from(n)),
        }
    }
}

/// A decimal or `0x` hex quantity (what `BigInt(...)` reads).
pub fn quantity(s: &str) -> Option<u128> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if hex.is_empty() {
            return Some(0);
        }
        let trimmed = hex.trim_start_matches('0');
        if trimmed.len() > 32 {
            return None;
        }
        return u128::from_str_radix(if trimmed.is_empty() { "0" } else { trimmed }, 16).ok();
    }
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// The chain did not answer: nothing about the multisig can be said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainUnavailable {
    pub chain_id: u64,
    pub detail: String,
}

/// Members read at a nonce: `(nonce, owners, threshold, read at)`.
type MembersAt = (u128, Vec<String>, u64, f64);

/// What this session read of chains, shared by a machine's commands.
#[derive(Clone, Debug, Default)]
pub struct Chain {
    /// `chainId:safe` seen deployed (code does not go away); kept on the
    /// device under [`keys::DEPLOYED`].
    pub deployed_seen: Shared<Vec<String>>,
    members_at: Shared<BTreeMap<String, MembersAt>>,
    contract_verdicts: Shared<BTreeMap<u64, multisig::SafeContracts>>,
}

fn word(hex: &str, index: usize) -> &str {
    let clean = hex.strip_prefix("0x").unwrap_or(hex);
    clean.get(index * 64..index * 64 + 64).unwrap_or("")
}

/// The addresses in an `address[]` return (`decodeAddresses`), checksummed.
/// A chain's answer for a deployed multisig's members is one only when it
/// names at least one owner and a threshold those owners can meet.
pub fn members_answer_valid(owners: &[String], threshold: u64) -> bool {
    !owners.is_empty() && threshold >= 1 && threshold <= owners.len() as u64
}

pub fn decode_addresses(hex: &str) -> Vec<String> {
    let offset = usize::from_str_radix(word(hex, 0), 16).unwrap_or(0) / 32;
    let count = usize::from_str_radix(word(hex, offset), 16).unwrap_or(0);
    (0..count.min(64))
        .map(|i| {
            let raw = format!("0x{}", word(hex, offset + 1 + i).get(24..).unwrap_or(""));
            checksummed(&raw)
        })
        .collect()
}

impl Chain {
    /// Seed what an earlier page kept (`vela.multisigDeployed`).
    pub fn seed_deployed(&self, raw: Option<&str>) {
        let kept: Vec<String> = raw
            .and_then(|r| serde_json::from_str::<Vec<Value>>(r).ok())
            .map(|v| {
                v.into_iter()
                    .filter_map(|k| k.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        *lock(&self.deployed_seen) = kept;
    }

    pub async fn has_code<Ev: Send + 'static>(
        io: &Io<Ev>,
        chain_id: u64,
        address: &str,
    ) -> Result<bool, ChainUnavailable> {
        Self::has_code_within(io, chain_id, address, None).await
    }

    pub async fn has_code_within<Ev: Send + 'static>(
        io: &Io<Ev>,
        chain_id: u64,
        address: &str,
        bound_ms: Option<u32>,
    ) -> Result<bool, ChainUnavailable> {
        let code = io
            .rpc_within(
                chain_id,
                "eth_getCode",
                json!([address, "latest"]),
                bound_ms,
            )
            .await
            .map_err(|e| ChainUnavailable {
                chain_id,
                detail: e.message(),
            })?;
        Ok(code.as_str().is_some_and(|c| c != "0x" && c != "0x0"))
    }

    /// Code there that is not an EIP-7702 delegation (`hasContractCode`).
    pub async fn has_contract_code<Ev: Send + 'static>(
        io: &Io<Ev>,
        chain_id: u64,
        address: &str,
    ) -> Result<bool, ChainUnavailable> {
        let code = io
            .rpc(chain_id, "eth_getCode", json!([address, "latest"]))
            .await
            .map_err(|e| ChainUnavailable {
                chain_id,
                detail: e.message(),
            })?;
        Ok(is_contract_code(&code))
    }

    /// Whether `chain_id` holds the Safe v1.4.1 contracts a new multisig
    /// needs; an `unknown` is asked again next time (`safeContractsOn`).
    pub async fn safe_contracts_on<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        chain_id: u64,
    ) -> multisig::SafeContracts {
        if let Some(known) = lock(&self.contract_verdicts).get(&chain_id).cloned() {
            return known;
        }
        let reads = multisig::MULTISIG_CONTRACTS
            .iter()
            .map(|(_, address)| async move {
                io.rpc(chain_id, "eth_getCode", json!([address, "latest"]))
                    .await
                    .ok()
                    .and_then(|c| c.as_str().map(str::to_owned))
            });
        let codes = futures::future::join_all(reads).await;
        let verdict = multisig::safe_contracts(&codes);
        if verdict != multisig::SafeContracts::Unknown {
            lock(&self.contract_verdicts).insert(chain_id, verdict.clone());
        }
        verdict
    }

    /// Whether `safe` is deployed on `chain_id`, remembered once seen.
    pub async fn deployed<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        chain_id: u64,
        safe: &str,
    ) -> Result<bool, ChainUnavailable> {
        let key = format!("{chain_id}:{}", safe.to_ascii_lowercase());
        if lock(&self.deployed_seen).contains(&key) {
            return Ok(true);
        }
        let yes = Self::has_code(io, chain_id, safe).await?;
        if yes {
            let kept = {
                let mut seen = lock(&self.deployed_seen);
                if !seen.contains(&key) {
                    seen.push(key);
                }
                let skip = seen.len().saturating_sub(DEPLOYED_CAP);
                seen[skip..].to_vec()
            };
            io.store(keys::DEPLOYED, serde_json::to_string(&kept).ok())
                .await;
        }
        Ok(yes)
    }

    /// The multisig's nonce on `chain_id` (0 where it is not deployed).
    pub async fn read_nonce<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        chain_id: u64,
        safe: &str,
    ) -> Result<u128, ChainUnavailable> {
        if !self.deployed(io, chain_id, safe).await? {
            return Ok(0);
        }
        let raw = io
            .eth_call(chain_id, safe, NONCE)
            .await
            .map_err(|e| ChainUnavailable {
                chain_id,
                detail: e.message(),
            })?;
        quantity(&raw).ok_or_else(|| ChainUnavailable {
            chain_id,
            detail: "nonce unreadable".to_owned(),
        })
    }

    /// Owners, threshold and nonce on `chain_id`, from the chain (or,
    /// undeployed, its genesis) — `readMembers`.
    pub async fn read_members<Ev: Send + 'static>(
        &self,
        io: &Io<Ev>,
        chain_id: u64,
        safe: &str,
        genesis: &Genesis,
    ) -> Result<ChainMembers, ChainUnavailable> {
        let unavailable = |detail: String| ChainUnavailable { chain_id, detail };
        if !self.deployed(io, chain_id, safe).await? {
            let (owners, threshold) =
                multisig::genesis_members(genesis).map_err(|e| unavailable(e.to_string()))?;
            return Ok(ChainMembers {
                deployed: false,
                owners,
                threshold,
                nonce: 0,
            });
        }
        let key = format!("{chain_id}:{}", safe.to_ascii_lowercase());
        let raw = io
            .eth_call(chain_id, safe, NONCE)
            .await
            .map_err(|e| unavailable(e.message()))?;
        let nonce = quantity(&raw).ok_or_else(|| unavailable("nonce unreadable".to_owned()))?;
        let now = io.now().await;
        if let Some((at_nonce, owners, threshold, at)) = lock(&self.members_at).get(&key).cloned() {
            if at_nonce == nonce && now - at < MEMBERS_FRESH_MS {
                return Ok(ChainMembers {
                    deployed: true,
                    owners,
                    threshold,
                    nonce,
                });
            }
        }
        let (owners, threshold) = futures::join!(
            io.eth_call(chain_id, safe, GET_OWNERS),
            io.eth_call(chain_id, safe, GET_THRESHOLD)
        );
        let owners = decode_addresses(&owners.map_err(|e| unavailable(e.message()))?);
        let threshold = quantity(&threshold.map_err(|e| unavailable(e.message()))?)
            .and_then(|t| u64::try_from(t).ok())
            .ok_or_else(|| unavailable("threshold unreadable".to_owned()))?;
        if !members_answer_valid(&owners, threshold) {
            // A deployed multisig always has a member and a rule it can
            // meet; "no owners, needs 0" is a node that answered nothing
            // (behind, pruned, or a chain that lost the deploy) — unread,
            // never "Needs 0 of 0" (r8-7).
            return Err(unavailable("members unreadable".to_owned()));
        }
        lock(&self.members_at).insert(key, (nonce, owners.clone(), threshold, now));
        Ok(ChainMembers {
            deployed: true,
            owners,
            threshold,
            nonce,
        })
    }
}

// ---------------------------------------------------------------------------
// Who can act for a multisig on a chain (`authority.ts`, `judge.ts`, §3.5)
// ---------------------------------------------------------------------------

/// An answer of the chain as a string, or `None` when it gave none.
async fn read_string<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    method: &str,
    params: Value,
) -> Option<String> {
    io.rpc(chain_id, method, params)
        .await
        .ok()
        .and_then(|r| r.as_str().map(str::to_owned))
}

/// Whether an `eth_getCode` answer is contract code (`hasContractCode`):
/// none, `0x`, and an EIP-7702 delegation (a person's wallet, which signs)
/// are not.
pub fn is_contract_code(code: &Value) -> bool {
    let Some(code) = code.as_str() else {
        return false;
    };
    if code == "0x" || code == "0x0" {
        return false;
    }
    let lower = code.to_ascii_lowercase();
    let delegated = lower.len() == 2 + 6 + 40
        && lower.starts_with("0xef0100")
        && lower[8..].bytes().all(|b| b.is_ascii_hexdigit());
    !delegated
}

/// `eth_getCode` of each address, as the core takes it.
pub async fn codes_of<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    addresses: &[String],
) -> BTreeMap<String, Option<String>> {
    let reads = addresses.iter().map(|a| async move {
        (
            a.clone(),
            read_string(io, chain_id, "eth_getCode", json!([a, "latest"])).await,
        )
    });
    futures::future::join_all(reads).await.into_iter().collect()
}

/// The code on `chain_id` of everything `genesis` depends on, read now —
/// never kept: the executor asks right before it deploys.
pub async fn counterfactual_facts<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    genesis: &Genesis,
) -> multisig::ChainFacts {
    let reads = multisig::counterfactual_reads(genesis, safe).unwrap_or_default();
    multisig::ChainFacts {
        codes: codes_of(io, chain_id, &reads).await,
    }
}

/// The core's verdict on `safe`, not deployed on `chain_id`, deployed there from `genesis`.
pub async fn counterfactual_on<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    genesis: &Genesis,
) -> Option<multisig::Authority> {
    let facts = counterfactual_facts(io, chain_id, safe, genesis).await;
    multisig::counterfactual_authority(genesis, safe, &facts).ok()
}

/// What can act for `safe`, deployed on `chain_id`, besides its members.
pub async fn deployed_on<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
) -> multisig::DeployedCheck {
    let reads = multisig::deployed_reads();
    let s = |k: &str| {
        reads
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let slot = |slot: String| async move {
        read_string(
            io,
            chain_id,
            "eth_getStorageAt",
            json!([safe, slot, "latest"]),
        )
        .await
    };
    let (modules, guard, module_guard, handler, singleton) = futures::join!(
        read_string(
            io,
            chain_id,
            "eth_call",
            json!([{ "to": safe, "data": s("modulesCall") }, "latest"])
        ),
        slot(s("guardSlot")),
        slot(s("moduleGuardSlot")),
        slot(s("handlerSlot")),
        slot(s("singletonSlot"))
    );
    let mut facts = multisig::DeployedFacts {
        modules,
        guard,
        module_guard,
        handler,
        singleton,
        codes: BTreeMap::new(),
    };
    let first = multisig::deployed_check(&facts);
    if first.need_code.is_empty() {
        return first;
    }
    facts.codes = codes_of(io, chain_id, &first.need_code).await;
    multisig::deployed_check(&facts)
}

/// A finding about a multisig on a chain, in the shape the web stores.
pub fn genesis_finding(chain_id: u64, reason: &multisig::Refusal) -> Value {
    json!({ "from": "genesis", "chainId": chain_id, "reason": reason })
}

pub fn chain_finding(chain_id: u64, risk: &multisig::Risk) -> Value {
    json!({ "from": "chain", "chainId": chain_id, "risk": risk })
}

/// What the chain says about `safe` on `chain_id` now, as findings: `[]` —
/// only its members act for it (or its genesis is Verified there); `None`
/// — the chain did not answer (`judgeOn`).
pub async fn judge_on<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    genesis: Option<&Genesis>,
    deployed: bool,
) -> Option<Vec<Value>> {
    if deployed {
        let check = deployed_on(io, chain_id, safe).await;
        if check.unread {
            return None;
        }
        return Some(
            check
                .risks
                .iter()
                .map(|r| chain_finding(chain_id, r))
                .collect(),
        );
    }
    let genesis = genesis?;
    match counterfactual_on(io, chain_id, safe, genesis).await? {
        multisig::Authority::Unknown { .. } => None,
        multisig::Authority::Refused { reasons } => Some(
            reasons
                .iter()
                .map(|r| genesis_finding(chain_id, r))
                .collect(),
        ),
        multisig::Authority::Verified { .. } => Some(Vec::new()),
    }
}

/// `judge` (`recheck.ts`): deployed or not, then [`judge_on`].
pub async fn judge<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    genesis: &Genesis,
) -> Option<Vec<Value>> {
    let deployed = Chain::has_code(io, chain_id, safe).await.ok()?;
    judge_on(io, chain_id, safe, Some(genesis), deployed).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_member_answer_is_unread_not_needs_zero() {
        let one = vec!["0x00000000000000000000000000000000000000aA".to_owned()];
        let two = vec![
            one[0].clone(),
            "0x00000000000000000000000000000000000000bB".to_owned(),
        ];
        assert!(members_answer_valid(&one, 1));
        assert!(members_answer_valid(&two, 2));
        // What a node returns for an address with no code: "0x".
        assert!(decode_addresses("0x").is_empty());
        assert!(!members_answer_valid(&decode_addresses("0x"), 0));
        assert!(!members_answer_valid(&[], 1));
        assert!(!members_answer_valid(&two, 0));
        assert!(!members_answer_valid(&two, 3));
    }

    #[test]
    fn retry_after_reads_the_header_then_the_body() {
        assert_eq!(retry_after_ms(Some("3"), ""), Some(3000.0));
        assert_eq!(retry_after_ms(Some(" 12 "), ""), Some(12000.0));
        assert_eq!(
            retry_after_ms(None, "too many requests; retry in 3 s"),
            Some(3000.0)
        );
        assert_eq!(retry_after_ms(None, "retry in 7s."), Some(7000.0));
        assert_eq!(retry_after_ms(Some("soon"), "retry in 2 s"), Some(2000.0));
        assert_eq!(retry_after_ms(None, "retry in 2 seconds"), None);
        assert_eq!(retry_after_ms(None, "busy"), None);
    }

    #[test]
    fn the_outage_steps_and_caps() {
        let mut o = Outage::default();
        assert_eq!(o.available_now(0.0), Some(true));
        o.failed(1000.0);
        assert!(o.down);
        assert_eq!(o.retry_at(), 16_000.0);
        assert!(o.blocked(15_999.0));
        // A request already out when it was marked counts once.
        o.failed(2000.0);
        assert_eq!(o.retry_at(), 16_000.0);
        assert_eq!(o.available_now(16_000.0), None, "one probe at the step");
        o.failed(16_000.0);
        assert_eq!(o.retry_at(), 46_000.0);
        for i in 0..10 {
            o.failed(1e9 + f64::from(i) * 1e7);
        }
        assert_eq!(o.retry_at() - (1e9 + 9.0 * 1e7), 300_000.0, "capped");
        o.answered();
        assert!(!o.down);
        o.busy(5000.0);
        assert_eq!(o.available_now(4000.0), Some(false));
        assert_eq!(o.available_now(5000.0), Some(true));
    }

    #[test]
    fn quantities_read_as_bigint_does() {
        assert_eq!(quantity("0x0"), Some(0));
        assert_eq!(quantity("0x"), Some(0));
        assert_eq!(quantity("0x1f"), Some(31));
        assert_eq!(quantity("42"), Some(42));
        assert_eq!(quantity("-1"), None);
        assert_eq!(quantity("0xzz"), None);
    }

    #[test]
    fn owners_decode_checksummed() {
        let ret = concat!(
            "0x",
            "0000000000000000000000000000000000000000000000000000000000000020",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "000000000000000000000000d400866e00b055b20752a826cd5c89b811de130b",
            "000000000000000000000000031d7d57c99caf891e1c250554691fd12d84772b"
        );
        assert_eq!(
            decode_addresses(ret),
            vec![
                "0xD400866e00B055B20752a826CD5C89b811de130b".to_owned(),
                "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned()
            ]
        );
    }

    #[test]
    fn a_stored_account_reads_and_writes_as_the_web_keeps_it() {
        let raw = r#"{"address":"0xabc","genesis":{"factory":"0x1","singleton":"0x2","saltNonce":"0x3","initializer":"0x4","name":"Quiet Harbor"},"owners":["0xa","0xb"],"threshold":2,"recorded":false,"shared":true,"chainIds":[100],"addedMs":1700000000000,"members":{"0xa":{"name":"Ana","kind":"passkey","typed":true}}}"#;
        let a: MultisigAccount = serde_json::from_str(raw).unwrap();
        assert!(a.is_shared());
        assert_eq!(a.chain_ids, vec![100]);
        let back = serde_json::to_value(&a).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(raw).unwrap());
    }
}
