//! The RPC pool — one routing authority for the whole process.
//!
//! ## Why this is a thread and not a gpui resident
//!
//! Every other machine in this client lives in [`crate::resident`], driven from
//! the main thread and rendered by a screen. The pool has neither property. Its
//! callers are background workers doing blocking HTTP — the balance fetch, the
//! activity read, a recipient probe — and they need an answer *on the thread
//! they are already on*. Routing them through the main thread would put a
//! multi-second network round trip in front of the next frame, which is the one
//! thing the resident host was built to avoid.
//!
//! So the pool owns a thread. Callers send a request and block on a reply; the
//! thread runs `CoreHost<RpcPool>` and performs the pool's own effects inline,
//! where blocking is free.
//!
//! ## One session, and why that is not a detail
//!
//! The ban map, the per-endpoint statistics and the fastest-RPC race winners are
//! **facts about the network that every caller shares**. Two sessions means an
//! endpoint banned for the balance fetch and retried by the activity read a
//! second later, and a ban map that disagrees with itself — which is the bug the
//! web port names in its own header. Hence one `OnceLock`, and no way to make a
//! second.
//!
//! ## What lives here and what does not
//!
//! This file owns exactly two things the core cannot: **the fetch**, and **the
//! reply channel the caller is waiting on**. Which endpoint to try, in what
//! order, after which failure, under which ban, for how long — six-tier source
//! scoring, EMA latency, cooldowns, temp and permanent bans, the four-way error
//! classification, the three-pass sweep, the all-banned self-rescue — is
//! `rpc_pool.rs`'s 1,975 lines and is not re-derived here. If this file grows an
//! `if` that decides where a call goes next, it is in the wrong file.
//!
//! Hedged reads (spec 083 H6) keep that rule: a read silent for 1.5 s also
//! asks the next endpoint, the caller takes the first answer the core's own
//! classifier accepts, and the core is still told every outcome in its own
//! order — see [`Hedge`].

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::network_admin::{
    BUILTIN_CHAINS, NetProviderId, PROVIDER_ORDER, build_provider_rpc_url,
};
use vela_core::app::rpc_pool::{
    Event, RPC_READ_TIMEOUT_MS, RpcBanEntry, RpcCallVerdict, RpcEndpointSeed, RpcKind,
    RpcOperation, RpcShellResult, RpcSource, RpcTransportOutcome, concluding_verdict,
    is_ban_active,
};

use crate::core_host::CoreHost;
use crate::executor::{proxy, storage};

/// Curated public fallbacks (`PUBLIC_RPCS`, rpc-pool-endpoints.ts:50-60).
///
/// A data table, ported verbatim. It is tier 4 of six — below a user override, a
/// configured provider and the built-in default, above whatever the chain index
/// happens to list.
const PUBLIC_RPCS: &[(u32, &[&str])] = &[
    (
        1,
        &["https://ethereum-rpc.publicnode.com", "https://1rpc.io/eth"],
    ),
    // bsc.meowrpc.com was dropped (issue #212; measured 2026-09-20): its
    // eth_gasPrice flips between 0.05, 0.1 and 1.0 gwei and ~33% of calls error.
    (
        56,
        &["https://bsc-rpc.publicnode.com", "https://bsc.drpc.org"],
    ),
    (
        137,
        &[
            "https://polygon-bor-rpc.publicnode.com",
            "https://1rpc.io/matic",
        ],
    ),
    (
        42161,
        &[
            "https://arbitrum-one-rpc.publicnode.com",
            "https://1rpc.io/arb",
        ],
    ),
    (
        10,
        &["https://optimism-rpc.publicnode.com", "https://1rpc.io/op"],
    ),
    (
        8453,
        &["https://base-rpc.publicnode.com", "https://1rpc.io/base"],
    ),
    (
        43114,
        &[
            "https://avalanche-c-chain-rpc.publicnode.com",
            "https://1rpc.io/avax/c",
        ],
    ),
    (
        100,
        &[
            "https://gnosis-rpc.publicnode.com",
            "https://1rpc.io/gnosis",
        ],
    ),
    (196, &["https://rpc.xlayer.tech", "https://xlayer.drpc.org"]),
];

/// How long a read waits on one endpoint before the same read also goes to
/// the next (spec 083 D3b, hand-off H6). A black-holed node used to hold a
/// dApp's read — and the wallet's own — for the core's whole 8 s read timeout
/// before the next node was asked at all.
const HEDGE_AFTER: Duration = Duration::from_millis(1_500);

/// The reads a hedge may repeat (083 H6). Each answers from chain state and
/// changes nothing, so asking two nodes costs one request and never a second
/// effect. Anything that writes — `eth_sendRawTransaction`, which a dApp's
/// page sends through [`call`] too — is absent and never hedged, and so are
/// the filter methods, whose ids live on one node only.
const HEDGED_READS: &[&str] = &[
    "eth_blockNumber",
    "eth_call",
    "eth_chainId",
    "eth_createAccessList",
    "eth_estimateGas",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByHash",
    "eth_getBlockByNumber",
    "eth_getBlockReceipts",
    "eth_getBlockTransactionCountByHash",
    "eth_getBlockTransactionCountByNumber",
    "eth_getCode",
    "eth_getLogs",
    "eth_getProof",
    "eth_getStorageAt",
    "eth_getTransactionByBlockHashAndIndex",
    "eth_getTransactionByBlockNumberAndIndex",
    "eth_getTransactionByHash",
    "eth_getTransactionCount",
    "eth_getTransactionReceipt",
    "eth_maxPriorityFeePerGas",
    "eth_simulateV1",
    "eth_syncing",
    "net_version",
    "web3_clientVersion",
];

/// What a caller asked for and where to send the answer.
enum Request {
    Call {
        chain_id: u32,
        kind: RpcKind,
        method: String,
        params: Value,
        reply: Sender<Result<Value, PoolError>>,
    },
    /// Drop every endpoint's state for a chain, or for all of them.
    Refresh { chain_id: Option<u32> },
    /// Which bundler REST base the pool would submit to (invariant ③).
    BundlerBase {
        chain_id: u32,
        reply: Sender<Option<String>>,
    },
    /// The chains whose last failure was the providers' rate limit.
    RateLimited { reply: Sender<Vec<u32>> },
    /// The chains whose whole RPC pool failed on the last attempt.
    Failed { reply: Sender<Vec<u32>> },
    /// How many routed calls the pool thread still holds: a test's proof
    /// that an answered hedge leaves nothing behind (083 H6).
    #[cfg(test)]
    Open { reply: Sender<usize> },
}

/// Why a routed call produced no answer.
#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    /// Every endpoint failed every pass. `rate_limited` is the self-healing
    /// transient case (invariant ④) and is worth showing differently.
    Failed { rate_limited: bool },
    /// `eth_getLogs` hit a range cap; the caller splits and retries.
    RangeCap { max_span: f64 },
    /// The pool thread is gone. Only reachable if it panicked.
    Unavailable,
}

static POOL: OnceLock<Mutex<Sender<Message>>> = OnceLock::new();

fn sender() -> &'static Mutex<Sender<Message>> {
    POOL.get_or_init(|| {
        let (tx, rx) = channel::<Message>();
        let workers = tx.clone();
        std::thread::Builder::new()
            .name("vela-rpc-pool".to_owned())
            .spawn(move || run(&rx, &workers))
            .ok();
        Mutex::new(tx)
    })
}

/// One routed JSON-RPC call. **Blocks** — call it from a worker, never a frame.
#[allow(
    dead_code,
    reason = "the read machines' entry point; wired by 031's next phase, and exercised by this module's live test"
)]
pub fn call(chain_id: u32, method: &str, params: Value) -> Result<Value, PoolError> {
    dispatch(chain_id, RpcKind::Rpc, method, params)
}

/// The same, against the chain's bundler rather than its RPC.
#[allow(dead_code, reason = "the money path's entry point, wired by spec 032")]
pub fn bundler_call(chain_id: u32, method: &str, params: Value) -> Result<Value, PoolError> {
    dispatch(chain_id, RpcKind::Bundler, method, params)
}

/// [`bundler_call`], waiting at most `budget` for the answer (spec 079): a
/// caller with a deadline of its own — the dApp's receipt wait — must not be
/// held past it by one call's timeouts and retries. The call itself runs on
/// to its end inside the pool (its verdicts still count); only this caller
/// stops waiting, and a late answer goes nowhere.
pub fn bundler_call_within(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    dispatch_within(chain_id, RpcKind::Bundler, method, params, budget)
}

/// [`call`], waiting at most `budget` — the node's half of the dApp's
/// landing wait (083: a bundle transaction's receipt, read to learn how the
/// operation inside it ended).
pub fn call_within(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    dispatch_within(chain_id, RpcKind::Rpc, method, params, budget)
}

fn dispatch_within(
    chain_id: u32,
    kind: RpcKind,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    let (reply, answer) = channel();
    {
        let Ok(tx) = sender().lock() else {
            return Err(PoolError::Unavailable);
        };
        if tx
            .send(Message::Ask(Request::Call {
                chain_id,
                kind,
                method: method.to_owned(),
                params,
                reply,
            }))
            .is_err()
        {
            return Err(PoolError::Unavailable);
        }
    }
    match answer.recv_timeout(budget) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(PoolError::Failed {
            rate_limited: false,
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(PoolError::Unavailable),
    }
}

/// The REST base of the bundler the pool would submit to for this chain
/// (`getActiveBundlerBaseUrl`, invariant ③): the `/v1/account`, `/v1/treasury`
/// and `/v1/sponsor` calls must reach the SAME relay the user operation goes
/// to, or a reimbursement recipient read from one relay is submitted to
/// another and refused. `None` when every bundler endpoint is banned or the
/// pool is empty — the caller falls back to the built-in base. **Blocks.**
pub fn bundler_base(chain_id: u32) -> Option<String> {
    query(chain_id, |chain_id, reply| Request::BundlerBase {
        chain_id,
        reply,
    })
}

fn query(
    chain_id: u32,
    make: impl FnOnce(u32, Sender<Option<String>>) -> Request,
) -> Option<String> {
    let (reply, answer) = channel();
    {
        let tx = sender().lock().ok()?;
        tx.send(Message::Ask(make(chain_id, reply))).ok()?;
    }
    answer.recv().ok().flatten()
}

/// The chains whose last failed call saw only rate limits (`getRateLimitedChains`,
/// invariant ④): transient — the balance keeps what it had and nobody is told
/// to swap in their own RPC (078 W-08). **Blocks** on the pool thread.
pub fn rate_limited_chains() -> Vec<u32> {
    let (reply, answer) = channel();
    let sent = sender().lock().ok().is_some_and(|tx| {
        tx.send(Message::Ask(Request::RateLimited { reply }))
            .is_ok()
    });
    if !sent {
        return Vec::new();
    }
    answer.recv().unwrap_or_default()
}

/// The chains whose whole RPC pool failed on the last attempt (`RpcPoolView.
/// failed_chains`; spec 079 US4) — the rate-limited ones among them too, which
/// a caller subtracts with [`rate_limited_chains`]. Cleared by the next usable
/// answer from any caller. **Blocks** on the pool thread, briefly: it is
/// answered from the view, never from the network.
pub fn failed_chains() -> Vec<u32> {
    let (reply, answer) = channel();
    let sent = sender()
        .lock()
        .ok()
        .is_some_and(|tx| tx.send(Message::Ask(Request::Failed { reply })).is_ok());
    if !sent {
        return Vec::new();
    }
    answer.recv().unwrap_or_default()
}

/// Forget an endpoint's measured state — after the settings screen edits it.
#[allow(dead_code, reason = "wired to network_admin's invalidate_pools next")]
pub fn refresh(chain_id: Option<u32>) {
    if let Ok(tx) = sender().lock() {
        let _ = tx.send(Message::Ask(Request::Refresh { chain_id }));
    }
}

fn dispatch(chain_id: u32, kind: RpcKind, method: &str, params: Value) -> Result<Value, PoolError> {
    let (reply, answer) = channel();
    {
        let Ok(tx) = sender().lock() else {
            return Err(PoolError::Unavailable);
        };
        if tx
            .send(Message::Ask(Request::Call {
                chain_id,
                kind,
                method: method.to_owned(),
                params,
                reply,
            }))
            .is_err()
        {
            return Err(PoolError::Unavailable);
        }
    }
    answer.recv().unwrap_or(Err(PoolError::Unavailable))
}

// ---------------------------------------------------------------------------
// The thread
// ---------------------------------------------------------------------------

/// What reaches the pool thread. Callers send `Ask`; workers send `Finished`.
///
/// One channel rather than two because the thread must be able to wait on
/// **either** without polling, and `mpsc` gives no select.
enum Message {
    Ask(Request),
    /// A blocking operation a worker thread finished. `body` is carried back
    /// rather than written by the worker: `inflight` belongs to the pool
    /// thread, and a body that crossed threads to reach it would be the one
    /// piece of shared mutable state this file exists to avoid.
    Finished {
        id: u64,
        result: RpcShellResult,
        body: Option<(String, String, Value)>,
    },
    /// A hedge came back (083 H6): a `PostOutcome` the core has not asked
    /// for yet, and the body it names.
    Hedged {
        result: RpcShellResult,
        body: Option<Value>,
    },
}

/// How many worker threads may be waiting on the network at once.
///
/// A ceiling, not a target: past it, the operation is performed on the pool
/// thread as it was before this phase. Twelve chains × a couple of reads is
/// the real peak, so the cap is never met in normal traffic — it is here so a
/// machine that loops degrades to slow rather than to a thousand threads.
const MAX_WORKERS: usize = 32;

static WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// What the shell is holding for one in-flight call.
struct InFlight {
    params: Value,
    /// Taken by whoever answers the caller first: the core's verdict, or a
    /// hedge whose answer the core would accept itself (083 H6).
    reply: Option<Sender<Result<Value, PoolError>>>,
    /// The last body received, per URL. `Conclude { Respond { url } }` names
    /// which one the core accepted — the core never sees a body itself.
    bodies: HashMap<String, Value>,
    /// Only a read in [`HEDGED_READS`] through the RPC pool has one.
    hedge: Option<Hedge>,
}

impl InFlight {
    /// When this call's next hedge is due; never once the caller has its
    /// answer.
    fn hedge_due(&self) -> Option<Instant> {
        self.reply.as_ref()?;
        self.hedge.as_ref()?.due()
    }
}

/// One read's hedging (spec 083 D3b, hand-off H6).
///
/// The core still walks its endpoints one at a time and is told every
/// outcome, truthfully, with its real latency, in its own order — its bans,
/// cooldowns and scores are exactly what they were. A hedge only asks an
/// endpoint EARLY: the caller takes the first answer the core would accept
/// itself, and when the core reaches that endpoint the outcome is already in
/// hand, so no endpoint is asked twice for one read. The loser is abandoned:
/// once the core concludes, a hedge that comes back later is dropped.
struct Hedge {
    chain_id: u32,
    method: String,
    /// The post the core is waiting on, and when it went out.
    current: Option<(String, Instant)>,
    /// Every endpoint this read has asked. None is hedged twice, which is
    /// what bounds a read to one extra request per endpoint in its pool.
    asked: HashSet<String>,
    /// Hedges on the wire: when each went out, and the core's operation id
    /// once the core has reached that endpoint and waits on it too.
    flying: HashMap<String, (Instant, Option<u64>)>,
    /// Hedges that came back before the core asked for their endpoint.
    landed: HashMap<String, RpcShellResult>,
    /// Nothing is left to hedge to, or no worker would take one.
    spent: bool,
}

impl Hedge {
    /// `Some` for a read that may be hedged; a write, and anything for the
    /// bundler, never is.
    fn for_call(chain_id: u32, kind: RpcKind, method: &str) -> Option<Self> {
        (kind == RpcKind::Rpc && HEDGED_READS.contains(&method)).then(|| Self {
            chain_id,
            method: method.to_owned(),
            current: None,
            asked: HashSet::new(),
            flying: HashMap::new(),
            landed: HashMap::new(),
            spent: false,
        })
    }

    /// The core's post has been out [`HEDGE_AFTER`] and no earlier hedge is
    /// still unanswered: one extra request on the wire per read, never a
    /// fan-out.
    fn due(&self) -> Option<Instant> {
        if self.spent || self.flying.values().any(|(_, core)| core.is_none()) {
            return None;
        }
        self.current.as_ref().map(|(_, sent)| *sent + HEDGE_AFTER)
    }

    /// The next endpoint in the pool's own tier order that this read has not
    /// asked and the core has not banned.
    fn target(
        &self,
        endpoints: &[RpcEndpointSeed],
        banned: &[RpcBanEntry],
        now_ms: f64,
    ) -> Option<String> {
        endpoints
            .iter()
            .map(|endpoint| &endpoint.url)
            .find(|url| {
                !self.asked.contains(*url)
                    && !banned
                        .iter()
                        .any(|ban| ban.url == **url && is_ban_active(ban, now_ms))
            })
            .cloned()
    }
}

fn run(rx: &std::sync::mpsc::Receiver<Message>, workers: &Sender<Message>) {
    let mut host = CoreHost::<RpcPoolApp>::new();
    let mut inflight: HashMap<String, InFlight> = HashMap::new();
    // The base-URL and best-RPC questions: no body, no params, one answer.
    let mut queries: HashMap<String, Sender<Option<String>>> = HashMap::new();
    let mut next_id: u64 = 0;

    // Bans persist across launches; a pool that forgot them would re-try an
    // endpoint the last session already proved dead.
    let entries = read_bans();
    let pending = host.dispatch(Event::BansLoaded { entries });
    drain(&mut host, pending, &mut inflight, &mut queries, workers);

    loop {
        // 083 H6: a read whose endpoint has been silent for `HEDGE_AFTER`
        // asks the next one. The wait below wakes for the earliest such read.
        hedge_due_reads(&host, &mut inflight, workers);
        let message = match inflight.values().filter_map(InFlight::hedge_due).min() {
            Some(due) => match rx.recv_timeout(due.saturating_duration_since(Instant::now())) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(message) => message,
                Err(_) => return,
            },
        };
        let request = match message {
            Message::Ask(request) => request,
            // A worker came back. The body it carried is filed under the call
            // it belongs to — if that call is still open: a concluded call is
            // gone, and a late body has nowhere to go, which is exactly right.
            Message::Finished { id, result, body } => {
                if let Some((call_id, url, body)) = body
                    && let Some(call) = inflight.get_mut(&call_id)
                {
                    call.bodies.insert(url, body);
                }
                post_answered(&mut inflight, &result);
                let pending = host.resolve(id, result);
                drain(&mut host, pending, &mut inflight, &mut queries, workers);
                continue;
            }
            Message::Hedged { result, body } => {
                if let Some((id, result)) = hedge_landed(&mut inflight, result, body) {
                    let pending = host.resolve(id, result);
                    drain(&mut host, pending, &mut inflight, &mut queries, workers);
                }
                continue;
            }
        };
        match request {
            Request::Call {
                chain_id,
                kind,
                method,
                params,
                reply,
            } => {
                next_id += 1;
                let call_id = format!("c{next_id}");
                inflight.insert(
                    call_id.clone(),
                    InFlight {
                        params,
                        reply: Some(reply),
                        bodies: HashMap::new(),
                        hedge: Hedge::for_call(chain_id, kind, &method),
                    },
                );
                let pending = host.dispatch(Event::CallRequested {
                    call_id,
                    chain_id,
                    kind,
                    method,
                    now_ms: now_ms(),
                });
                drain(&mut host, pending, &mut inflight, &mut queries, workers);
            }
            Request::Refresh { chain_id } => {
                let event = match chain_id {
                    Some(chain_id) => Event::RefreshChain { chain_id },
                    None => Event::InvalidateAll,
                };
                let pending = host.dispatch(event);
                drain(&mut host, pending, &mut inflight, &mut queries, workers);
            }
            Request::BundlerBase { chain_id, reply } => {
                next_id += 1;
                let call_id = format!("b{next_id}");
                queries.insert(call_id.clone(), reply);
                let pending = host.dispatch(Event::BundlerBaseRequested {
                    call_id,
                    chain_id,
                    now_ms: now_ms(),
                });
                drain(&mut host, pending, &mut inflight, &mut queries, workers);
            }
            Request::RateLimited { reply } => {
                let _ = reply.send(host.view().rate_limited_chains);
            }
            Request::Failed { reply } => {
                let _ = reply.send(host.view().failed_chains);
            }
            #[cfg(test)]
            Request::Open { reply } => {
                let _ = reply.send(inflight.len());
            }
        }
    }
}

type RpcPoolApp = vela_core::app::rpc_pool::RpcPool;

/// Perform what the core asked for, and resolve what came back.
///
/// **The waiting happens off this thread.** Anything that blocks on a socket
/// or a clock is handed to a worker and reported back through the channel;
/// only the cheap, local operations are answered here. That is the difference
/// between a pool that routes and a pool that is also a queue of one: before
/// this phase an eight-second timeout on a Optimism balance read held every
/// other caller in the process — the signing sheet's `decimals()` probe among
/// them, which has a four-second budget and was losing it to traffic it has
/// nothing to do with.
///
/// It is also what makes the core's endpoint RACE real. `rpc_pool` has always
/// been able to return several posts at once; performing them one after
/// another turned a race into a sequence, so the "fastest endpoint" it
/// measured was really "the first endpoint, plus however long the ones before
/// it took".
fn drain(
    host: &mut CoreHost<RpcPoolApp>,
    pending: Vec<crate::core_host::Pending<RpcOperation>>,
    inflight: &mut HashMap<String, InFlight>,
    queries: &mut HashMap<String, Sender<Option<String>>>,
    workers: &Sender<Message>,
) {
    // In the order the core listed them. It was a `Vec::pop` before this
    // phase, which ran the core's list BACKWARDS: when the core offered
    // several endpoints it had already scored them best-first, and the shell
    // reached for the worst one first. Nothing was wrong in the end — the
    // verdict is the core's and it names the URL it accepts — but "which
    // endpoint a call goes to first" is a routing decision, and this file's
    // own header says it does not make those.
    let mut queue: std::collections::VecDeque<_> = pending.into_iter().collect();
    while let Some(next) = queue.pop_front() {
        // 083 H6: the core reached an endpoint a hedge already asked. Its
        // answer is that hedge's — in hand, or on the wire — never a second
        // request.
        if let RpcOperation::JsonRpcPost { call_id, url, .. } = &next.operation
            && let Some(hedge) = inflight
                .get_mut(call_id)
                .and_then(|call| call.hedge.as_mut())
        {
            if let Some(result) = hedge.landed.remove(url) {
                queue.extend(host.resolve(next.id, result));
                continue;
            }
            if let Some((sent, core)) = hedge.flying.get_mut(url) {
                *core = Some(next.id);
                hedge.current = Some((url.clone(), *sent));
                continue;
            }
            hedge.asked.insert(url.clone());
            hedge.current = Some((url.clone(), Instant::now()));
        }
        if offload(next.id, &next.operation, inflight, workers) {
            continue;
        }
        let result = perform(&next.operation, inflight, queries);
        post_answered(inflight, &result);
        queue.extend(host.resolve(next.id, result));
    }
}

/// The core's own post came back: nothing of it is left to hedge (083 H6).
fn post_answered(inflight: &mut HashMap<String, InFlight>, result: &RpcShellResult) {
    if let RpcShellResult::PostOutcome { call_id, url, .. } = result
        && let Some(hedge) = inflight
            .get_mut(call_id)
            .and_then(|call| call.hedge.as_mut())
        && hedge
            .current
            .as_ref()
            .is_some_and(|(current, _)| current == url)
    {
        hedge.current = None;
    }
}

/// Send the next endpoint every read whose current one has been silent for
/// [`HEDGE_AFTER`] (083 H6).
fn hedge_due_reads(
    host: &CoreHost<RpcPoolApp>,
    inflight: &mut HashMap<String, InFlight>,
    workers: &Sender<Message>,
) {
    let now = Instant::now();
    // The core's ban truth, read once per round and only when a read is due.
    let mut banned: Option<Vec<RpcBanEntry>> = None;
    for (call_id, call) in inflight.iter_mut() {
        if !call.hedge_due().is_some_and(|due| due <= now) {
            continue;
        }
        let banned = banned.get_or_insert_with(|| host.view().banned);
        let Some(hedge) = call.hedge.as_mut() else {
            continue;
        };
        // The same six tiers `LoadPoolConfig` hands the core.
        let (endpoints, _) = collect_endpoints(hedge.chain_id);
        let Some(url) = hedge.target(&endpoints, banned, now_ms()) else {
            hedge.spent = true;
            continue;
        };
        let (call_id, target, method, params) = (
            call_id.clone(),
            url.clone(),
            hedge.method.clone(),
            call.params.clone(),
        );
        let work = Box::new(move || {
            let started = Instant::now();
            // The core's own read timeout: a hedge is the same post, early.
            let (outcome, body) = post(&target, &method, &params, None, RPC_READ_TIMEOUT_MS);
            Message::Hedged {
                body,
                result: RpcShellResult::PostOutcome {
                    call_id,
                    url: target,
                    outcome,
                    latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                    now_ms: now_ms(),
                },
            }
        });
        // At the worker ceiling a hedge is not sent; the core's sweep still
        // answers, as it did before hedging.
        if !spawn_worker(workers, work) {
            hedge.spent = true;
            continue;
        }
        hedge.asked.insert(url.clone());
        hedge.flying.insert(url, (now, None));
    }
}

/// A hedge came back (083 H6). The caller has it now if the core would
/// accept it; the core has it when it reaches that endpoint — at once, as
/// the returned operation, when it already waits there.
fn hedge_landed(
    inflight: &mut HashMap<String, InFlight>,
    result: RpcShellResult,
    body: Option<Value>,
) -> Option<(u64, RpcShellResult)> {
    let RpcShellResult::PostOutcome {
        call_id,
        url,
        outcome,
        ..
    } = &result
    else {
        return None;
    };
    // A concluded call is gone: the loser is abandoned, its answer ignored.
    let call = inflight.get_mut(call_id)?;
    let hedge = call.hedge.as_mut()?;
    let (_, core) = hedge.flying.remove(url)?;
    if let Some(body) = body {
        call.bodies.insert(url.clone(), body);
    }
    if let Some(verdict) = concluding_verdict(RpcKind::Rpc, &hedge.method, url, outcome)
        && let Some(reply) = call.reply.take()
    {
        let _ = reply.send(answer(&verdict, &call.bodies));
    }
    match core {
        Some(id) => {
            hedge.current = None;
            Some((id, result))
        }
        None => {
            hedge.landed.insert(url.clone(), result);
            None
        }
    }
}

/// Hand a blocking operation to a worker thread. `true` when one took it.
///
/// `false` means the caller performs it inline — either because it does not
/// block (routing decisions, the jitter draw, the verdict) or because the
/// ceiling is reached, in which case the old behaviour is the degradation.
fn offload(
    id: u64,
    operation: &RpcOperation,
    inflight: &HashMap<String, InFlight>,
    workers: &Sender<Message>,
) -> bool {
    let work: Box<dyn FnOnce() -> Message + Send> = match operation {
        RpcOperation::JsonRpcPost {
            call_id,
            url,
            method,
            x_rpc_url,
            timeout_ms,
        } => {
            // The params are read here, on the thread that owns `inflight`.
            let params = inflight
                .get(call_id)
                .map_or(Value::Array(Vec::new()), |call| call.params.clone());
            let (call_id, url, method, x_rpc_url, timeout_ms) = (
                call_id.clone(),
                url.clone(),
                method.clone(),
                x_rpc_url.clone(),
                *timeout_ms,
            );
            Box::new(move || {
                let started = Instant::now();
                let (outcome, body) =
                    post(&url, &method, &params, x_rpc_url.as_deref(), timeout_ms);
                Message::Finished {
                    id,
                    body: body.map(|body| (call_id.clone(), url.clone(), body)),
                    result: RpcShellResult::PostOutcome {
                        call_id,
                        url,
                        outcome,
                        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                        now_ms: now_ms(),
                    },
                }
            })
        }

        RpcOperation::ProbeChainId {
            chain_id,
            url,
            timeout_ms,
        } => {
            let (chain_id, url, timeout_ms) = (*chain_id, url.clone(), *timeout_ms);
            Box::new(move || {
                let started = Instant::now();
                let (_, body) = post(&url, "eth_chainId", &json!([]), None, timeout_ms);
                Message::Finished {
                    id,
                    body: None,
                    result: RpcShellResult::ChainIdProbed {
                        chain_id,
                        url,
                        reported: chain_id_of(body.as_ref()),
                        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                        now_ms: now_ms(),
                    },
                }
            })
        }

        // A backoff is a sleep, and a sleep on the pool thread is every other
        // caller waiting out a delay the core imposed on ONE endpoint.
        RpcOperation::StartBackoff { call_id, delay_ms } => {
            let (call_id, delay_ms) = (call_id.clone(), *delay_ms);
            Box::new(move || {
                std::thread::sleep(Duration::from_millis(u64::from(delay_ms)));
                Message::Finished {
                    id,
                    body: None,
                    result: RpcShellResult::BackoffElapsed {
                        call_id,
                        now_ms: now_ms(),
                    },
                }
            })
        }

        _ => return false,
    };
    spawn_worker(workers, work)
}

/// Run `work` on a worker thread that reports back through the channel.
/// `false` at the ceiling, or when the OS refuses a thread.
fn spawn_worker(workers: &Sender<Message>, work: Box<dyn FnOnce() -> Message + Send>) -> bool {
    use std::sync::atomic::Ordering;

    if WORKERS.load(Ordering::Relaxed) >= MAX_WORKERS {
        return false;
    }
    WORKERS.fetch_add(1, Ordering::Relaxed);
    let workers = workers.clone();
    let spawned = std::thread::Builder::new()
        .name("vela-rpc-work".to_owned())
        .spawn(move || {
            let message = work();
            WORKERS.fetch_sub(1, Ordering::Relaxed);
            // The pool thread is gone only if it panicked; there is nobody
            // left to tell, and the caller's own reply channel closing is
            // what reports it.
            let _ = workers.send(message);
        });
    if spawned.is_err() {
        WORKERS.fetch_sub(1, Ordering::Relaxed);
        return false;
    }
    true
}

/// The chain id an `eth_chainId` body reports, if it reported one.
fn chain_id_of(body: Option<&Value>) -> Option<u32> {
    body.and_then(|value| value.get("result"))
        .and_then(Value::as_str)
        .and_then(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
}

/// The operations the pool thread answers itself.
///
/// The three blocking ones are still here because [`offload`] can decline —
/// at the worker ceiling, or if the OS refuses a thread — and when it does,
/// this is the behaviour that was there before: correct, and serial.
fn perform(
    operation: &RpcOperation,
    inflight: &mut HashMap<String, InFlight>,
    queries: &mut HashMap<String, Sender<Option<String>>>,
) -> RpcShellResult {
    match operation {
        RpcOperation::LoadPoolConfig { chain_id } => {
            let (rpc_endpoints, bundler_endpoints) = collect_endpoints(*chain_id);
            RpcShellResult::PoolConfig {
                chain_id: *chain_id,
                rpc_endpoints,
                bundler_endpoints,
                now_ms: now_ms(),
            }
        }

        RpcOperation::JsonRpcPost {
            call_id,
            url,
            method,
            x_rpc_url,
            timeout_ms,
        } => {
            let params = inflight
                .get(call_id)
                .map_or(Value::Array(Vec::new()), |call| call.params.clone());
            let started = Instant::now();
            let (outcome, body) = post(url, method, &params, x_rpc_url.as_deref(), *timeout_ms);
            if let (Some(body), Some(call)) = (body, inflight.get_mut(call_id)) {
                // Held for `Conclude`. The core classifies from the `error`
                // member alone and never sees a body — which is what keeps a
                // 3 MB `eth_getLogs` answer out of its state.
                call.bodies.insert(url.clone(), body);
            }
            RpcShellResult::PostOutcome {
                call_id: call_id.clone(),
                url: url.clone(),
                outcome,
                latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                now_ms: now_ms(),
            }
        }

        RpcOperation::ProbeChainId {
            chain_id,
            url,
            timeout_ms,
        } => {
            let started = Instant::now();
            let (_, body) = post(url, "eth_chainId", &json!([]), None, *timeout_ms);
            RpcShellResult::ChainIdProbed {
                chain_id: *chain_id,
                url: url.clone(),
                reported: chain_id_of(body.as_ref()),
                latency_ms: started.elapsed().as_secs_f64() * 1000.0,
                now_ms: now_ms(),
            }
        }

        // All randomness is injected. The core is a pure function of its inputs,
        // which is why its jitter is a question rather than a `rand::random`.
        RpcOperation::DrawJitter { call_id } => RpcShellResult::Jitter {
            call_id: call_id.clone(),
            value: unit_random(),
        },

        RpcOperation::StartBackoff { call_id, delay_ms } => {
            std::thread::sleep(Duration::from_millis(u64::from(*delay_ms)));
            RpcShellResult::BackoffElapsed {
                call_id: call_id.clone(),
                now_ms: now_ms(),
            }
        }

        RpcOperation::PersistBans { entries } => {
            write_bans(entries);
            RpcShellResult::Persisted
        }

        RpcOperation::Conclude { call_id, verdict } => {
            // The two questions that are not routed calls: answered from the
            // query table, never from a body.
            if let RpcCallVerdict::BundlerBase { base_url: answer }
            | RpcCallVerdict::BestRpcUrl { url: answer } = verdict
            {
                if let Some(reply) = queries.remove(call_id) {
                    let _ = reply.send(answer.clone());
                }
                return RpcShellResult::Concluded;
            }
            // A caller a hedge already answered has no reply left (083 H6);
            // the core's verdict still closed the call, and its books.
            if let Some(call) = inflight.remove(call_id)
                && let Some(reply) = call.reply
            {
                // A caller that gave up is not an error: the receiver is simply
                // gone, and the pool has nothing to be sad about.
                let _ = reply.send(answer(verdict, &call.bodies));
            }
            RpcShellResult::Concluded
        }
    }
}

/// What a caller receives for a verdict — from the core's conclusion, or
/// from a hedge the core would have accepted (083 H6).
fn answer(verdict: &RpcCallVerdict, bodies: &HashMap<String, Value>) -> Result<Value, PoolError> {
    match verdict {
        RpcCallVerdict::Respond { url } => bodies.get(url).cloned().ok_or(PoolError::Failed {
            rate_limited: false,
        }),
        RpcCallVerdict::RangeCap { max_span, .. } => Err(PoolError::RangeCap {
            max_span: *max_span,
        }),
        RpcCallVerdict::Failed { rate_limited } => Err(PoolError::Failed {
            rate_limited: *rate_limited,
        }),
        // Not answers to a routed call; a caller waiting on one of these
        // asked the wrong question.
        RpcCallVerdict::BundlerBase { .. } | RpcCallVerdict::BestRpcUrl { .. } => {
            Err(PoolError::Failed {
                rate_limited: false,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The two things the core cannot have
// ---------------------------------------------------------------------------

/// One POST, classified into the five outcomes the core distinguishes.
///
/// The body comes back separately: the core is told only whether there was an
/// `error` member, because classification is its job and payload size is not.
fn post(
    url: &str,
    method: &str,
    params: &Value,
    x_rpc_url: Option<&str>,
    timeout_ms: u32,
) -> (RpcTransportOutcome, Option<Value>) {
    let payload = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    // Per URL: an endpoint on this machine is never reached through a proxy;
    // everything else walks the candidate chain (spec 038 Part B, T028), so a
    // dead proxy in the environment does not get every RPC endpoint banned.
    let timeout = Duration::from_millis(u64::from(timeout_ms));
    let mut response = match proxy::with_candidates_for(url, timeout, |agent| {
        // Spec 081 FR-007: the wallet no longer tells the relay which RPC endpoint it prefers. That header carried the user's first-choice URL, which can contain a provider API key — and the relay never read this name anyway (it reads `x-vela-rpc-url`), so nothing depended on it.
        let _ = x_rpc_url;
        agent
            .post(url)
            .header("content-type", "application/json")
            .send_json(&payload)
    }) {
        Ok(response) => response,
        Err(failure) => {
            return match failure.error {
                ureq::Error::StatusCode(status) => {
                    (RpcTransportOutcome::HttpError { status }, None)
                }
                ureq::Error::Timeout(_) => (RpcTransportOutcome::Timeout, None),
                _ => (RpcTransportOutcome::Network, None),
            };
        }
    };

    let mut text = String::new();
    if response
        .body_mut()
        .as_reader()
        .read_to_string(&mut text)
        .is_err()
    {
        return (RpcTransportOutcome::Network, None);
    }
    let Ok(body) = serde_json::from_str::<Value>(&text) else {
        return (RpcTransportOutcome::NonJson, None);
    };
    if !body.is_object() {
        return (RpcTransportOutcome::NonJson, None);
    }

    let error = body.get("error").and_then(|error| {
        Some(vela_core::app::rpc_pool::RpcErrorInfo {
            code: error
                .get("code")
                .and_then(Value::as_i64)
                .and_then(|code| i32::try_from(code).ok()),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    });
    (RpcTransportOutcome::Response { error }, Some(body))
}

/// The six tiers, in order (`collectRpcUrls`, rpc-pool-endpoints.ts:68-127).
///
/// Bans are NOT filtered here — they are the core's state, and it says so:
/// "Do NOT filter banned URLs — bans are this core's state."
fn collect_endpoints(chain_id: u32) -> (Vec<RpcEndpointSeed>, Vec<RpcEndpointSeed>) {
    let mut rpc = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |url: String, source: RpcSource, into: &mut Vec<RpcEndpointSeed>| {
        if url.is_empty() || !seen.insert(url.clone()) {
            return;
        }
        into.push(RpcEndpointSeed { url, source });
    };

    let builtin = BUILTIN_CHAINS.iter().find(|c| c.chain_id == chain_id);

    // 1. a per-network override, but only when it differs from the default.
    if let Some(config) = stored_network_config(chain_id)
        && !config.is_empty()
        && builtin.is_none_or(|c| c.rpc_url != config)
    {
        add(config, RpcSource::User, &mut rpc);
    }

    // 2. configured providers, in the canonical order.
    let keys = stored_provider_keys();
    for id in PROVIDER_ORDER {
        if let Some((_, key)) = keys.iter().find(|(provider, _)| *provider == id)
            && let Some(url) = build_provider_rpc_url(id, chain_id, key)
        {
            add(url, RpcSource::Provider, &mut rpc);
        }
    }

    // 3. the built-in default, then a custom network's own endpoint.
    if let Some(chain) = builtin {
        add(chain.rpc_url.to_owned(), RpcSource::Default, &mut rpc);
    }
    if let Some(custom) = stored_custom_rpc(chain_id) {
        add(custom, RpcSource::Default, &mut rpc);
    }

    // 4. the curated public fallbacks.
    if let Some((_, urls)) = PUBLIC_RPCS.iter().find(|(id, _)| *id == chain_id) {
        for url in *urls {
            add((*url).to_owned(), RpcSource::Public, &mut rpc);
        }
    }

    // Tiers 5 and 6 are the chain index, which this cut does not fetch here:
    // `LoadPoolConfig` is answered synchronously on the pool thread and an
    // index round trip would stall every first call on a chain behind it. The
    // index tiers are a recorded debt, not an oversight — the core orders
    // whatever it is given, so adding them later changes no rule.

    // The bundler is one URL per chain, from the relay, plus a custom override.
    let mut bundler = Vec::new();
    if let Some(url) = stored_custom_bundler(chain_id) {
        add(url, RpcSource::User, &mut bundler);
    }
    // `builtin_base()`, NOT the constant: Settings > Service nodes > Vela
    // Relay writes `bundlerServiceURL`, and reading past it here is what made
    // that field inert — every user operation still went to the shipped relay.
    add(
        format!("{}/{chain_id}", crate::executor::relay::builtin_base()),
        RpcSource::Default,
        &mut bundler,
    );

    (rpc, bundler)
}

fn now_ms() -> f64 {
    crate::executor::now_ms()
}

/// A uniform draw in [0,1). The pool's jitter, injected rather than generated
/// in the core.
fn unit_random() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    f64::from(nanos) / f64::from(1_000_000_000u32)
}

fn read_bans() -> Vec<RpcBanEntry> {
    let Ok(Some(Value::Array(items))) = storage::read_value(storage::KEY_RPC_BANNED) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<RpcBanEntry>(item).ok())
        .collect()
}

fn write_bans(entries: &[RpcBanEntry]) {
    let encoded = serde_json::to_value(entries).unwrap_or(Value::Null);
    let _ = storage::write_value(storage::KEY_RPC_BANNED, encoded);
}

fn stored_network_config(chain_id: u32) -> Option<String> {
    stored_array(storage::KEY_NETWORK_CONFIG, chain_id, "rpcURL")
}

fn stored_custom_rpc(chain_id: u32) -> Option<String> {
    stored_array(storage::KEY_CUSTOM_NETWORKS, chain_id, "rpcURL")
}

/// `isUsingBuiltinBundler`: no user-set bundler for this chain, or one that
/// is the built-in relay's host anyway. Only the built-in relay keeps a
/// per-Safe gas account that can run short, so only it is pre-checked.
pub fn uses_builtin_bundler(chain_id: u32) -> bool {
    let builtin = crate::executor::relay::builtin_base();
    stored_custom_bundler(chain_id).is_none_or(|url| url.contains(&builtin))
}

fn stored_custom_bundler(chain_id: u32) -> Option<String> {
    stored_array(storage::KEY_CUSTOM_NETWORKS, chain_id, "bundlerURL")
}

fn stored_array(key: &str, chain_id: u32, field: &str) -> Option<String> {
    let Ok(Some(Value::Array(items))) = storage::read_value(key) else {
        return None;
    };
    items.into_iter().find_map(|item| {
        (item.get("chainId").and_then(Value::as_u64) == Some(u64::from(chain_id)))
            .then(|| item.get(field).and_then(Value::as_str).map(str::to_owned))
            .flatten()
            .filter(|url| !url.is_empty())
    })
}

fn stored_provider_keys() -> Vec<(NetProviderId, String)> {
    let mut out = Vec::new();
    let Ok(Some(Value::Object(map))) = storage::read_value(storage::KEY_RPC_PROVIDERS) else {
        return out;
    };
    for (id, field) in [
        (NetProviderId::Alchemy, "alchemy"),
        (NetProviderId::Drpc, "drpc"),
        (NetProviderId::Ankr, "ankr"),
    ] {
        if let Some(key) = map.get(field).and_then(Value::as_str)
            && !key.is_empty()
        {
            out.push((id, key.to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The six tiers, in order, with bans deliberately NOT filtered.
    #[test]
    fn endpoints_are_collected_in_source_order() {
        storage::tests::with_temp_state("pool-tiers", || {
            let (rpc, bundler) = collect_endpoints(100);
            let sources: Vec<RpcSource> = rpc.iter().map(|e| e.source).collect();

            // With no override and no provider key, the first tier present is
            // the built-in default, then the curated public list.
            assert_eq!(sources.first(), Some(&RpcSource::Default));
            assert!(
                sources.iter().any(|s| *s == RpcSource::Public),
                "the curated fallbacks must be offered: {sources:?}"
            );
            assert!(
                rpc.iter().any(|e| e.url == "https://rpc.gnosischain.com"),
                "Gnosis's built-in endpoint must be in the pool"
            );

            // Deduped by URL: the built-in and the public list overlap.
            let mut urls: Vec<&str> = rpc.iter().map(|e| e.url.as_str()).collect();
            let count = urls.len();
            urls.sort_unstable();
            urls.dedup();
            assert_eq!(urls.len(), count, "an endpoint was offered twice");

            assert!(
                bundler.iter().any(|e| e.url.ends_with("/100")),
                "the relay's chain base must be the bundler endpoint"
            );
        });
    }

    /// The bundler tier follows Settings > Service nodes > Vela Relay.
    ///
    /// It used to read `relay::BUILTIN_BASE` — so the field saved, probed and
    /// showed a latency badge, and every user operation still went to the
    /// shipped relay. The pool is the only thing that decides where a bundler
    /// call lands, so this is where "configured" has to be true.
    #[test]
    fn the_bundler_tier_follows_the_configured_relay() {
        storage::tests::with_temp_state("pool-relay", || {
            let (_, shipped) = collect_endpoints(100);
            assert_eq!(
                shipped.first().map(|e| e.url.as_str()),
                Some(format!("{}/100", crate::executor::relay::BUILTIN_BASE).as_str())
            );

            if storage::write_value(
                storage::KEY_SERVICE_ENDPOINTS,
                json!({ "bundlerServiceURL": "https://my-relay.example/" }),
            )
            .is_err()
            {
                unreachable!("could not seed");
            }
            let (_, configured) = collect_endpoints(100);
            assert_eq!(
                configured.first().map(|e| e.url.as_str()),
                Some("https://my-relay.example/100"),
                "the configured relay, with its trailing slash folded in"
            );
        });
    }

    /// A configured override outranks everything, and only when it DIFFERS from
    /// the default — "override" and "the same value" are not the same fact, and
    /// the tier is what the core scores on.
    #[test]
    fn an_override_equal_to_the_default_is_not_a_user_tier() {
        storage::tests::with_temp_state("pool-override", || {
            let builtin = BUILTIN_CHAINS
                .iter()
                .find(|c| c.chain_id == 100)
                .unwrap_or_else(|| unreachable!("Gnosis is built in"));

            let same = json!([{ "chainId": 100, "rpcURL": builtin.rpc_url }]);
            if storage::write_value(storage::KEY_NETWORK_CONFIG, same).is_err() {
                unreachable!("could not seed");
            }
            let (rpc, _) = collect_endpoints(100);
            assert!(
                !rpc.iter().any(|e| e.source == RpcSource::User),
                "an override identical to the default is not an override"
            );

            let different = json!([{ "chainId": 100, "rpcURL": "https://my.node.example" }]);
            if storage::write_value(storage::KEY_NETWORK_CONFIG, different).is_err() {
                unreachable!("could not seed");
            }
            let (rpc, _) = collect_endpoints(100);
            assert_eq!(rpc.first().map(|e| e.source), Some(RpcSource::User));
            assert_eq!(
                rpc.first().map(|e| e.url.as_str()),
                Some("https://my.node.example")
            );
        });
    }

    /// Bans round-trip through the same key every other client uses.
    #[test]
    fn bans_persist_under_the_shared_key() {
        storage::tests::with_temp_state("pool-bans", || {
            assert!(read_bans().is_empty());
            write_bans(&[RpcBanEntry {
                url: "https://dead.example".to_owned(),
                banned_at_ms: 1_756_000_000_000.0,
                permanent: true,
            }]);
            let raw = storage::read_value(storage::KEY_RPC_BANNED)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("nothing written"));
            assert!(raw.is_array(), "the ban map is a list of entries");
            let back = read_bans();
            assert_eq!(back.len(), 1);
            assert!(back[0].permanent);
        });
    }

    /// SC-002's first half, and it is structural rather than empirical: there
    /// is exactly ONE session, and no way to ask for a second.
    ///
    /// `sender()` hands back a `&'static Mutex` from a `OnceLock`, so pointer
    /// identity is the whole proof — every caller in this process, from every
    /// machine, is talking to the same thread and therefore to the same ban
    /// map, endpoint statistics and race winners. The bug this forecloses is
    /// the one the web port names in its own header: two sessions, an endpoint
    /// banned by the balance fetch and retried by the activity read a second
    /// later, and a ban map that disagrees with itself.
    #[test]
    fn one_session_serves_every_caller() {
        assert!(
            std::ptr::eq(sender(), sender()),
            "a second pool session exists"
        );
    }

    /// SC-002's second half: a ban one machine's read earns is in the state the
    /// next machine's read meets.
    ///
    /// Chain 100's built-in default endpoint answers this HTTP client with 403
    /// while curl gets 200 — a debt 030 recorded with no fix, and the reason
    /// this is testable at all. The balance service reads Gnosis; the pool
    /// meets that endpoint, classifies it and bans it. Then a DIFFERENT
    /// machine's service reads the same chain and finds the ban already there.
    #[test]
    #[ignore = "hits the real Gnosis pool"]
    fn a_ban_one_machine_earns_is_the_ban_the_next_machine_meets() {
        storage::tests::with_temp_state("pool-shared-ban", || {
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            assert!(read_bans().is_empty(), "a fresh state directory");

            // Machine one: the balance dashboard's service.
            let first = crate::executor::balances::fetch_all(GOLDEN);
            assert!(!first.0.is_empty(), "nothing was read at all");
            let after_first = read_bans();
            for entry in &after_first {
                println!("  banned by the balance read: {}", entry.url);
            }
            assert!(
                !after_first.is_empty(),
                "the 403 endpoint should have been banned"
            );

            // Machine two: the price service, a different caller entirely.
            crate::executor::chainlink::invalidate();
            let prices = crate::executor::chainlink::prices();
            assert!(!prices.is_empty(), "the price read got nothing");

            // Every ban the first read earned is still known. It was not
            // rediscovered from an empty map, which is what a second session
            // would have forced.
            let after_second = read_bans();
            for entry in &after_first {
                assert!(
                    after_second.iter().any(|later| later.url == entry.url),
                    "{} was forgotten between callers",
                    entry.url
                );
            }
            println!(
                "  {} ban(s) survived across two machines' reads",
                after_first.len()
            );
        });
    }

    /// The defect this phase closes: one slow endpoint used to hold every
    /// other caller in the process.
    ///
    /// Two chains, two local servers, one deliberately slow. The slow call
    /// starts first; the fast one must come back while it is still waiting.
    /// Before this phase both went through the pool thread's own `drain`, so
    /// the fast answer could not arrive until the slow one had — which is
    /// what put a `decimals()` probe with a four-second budget behind an
    /// eight-second balance timeout on a chain the signing sheet never asked
    /// about.
    ///
    /// The threshold is deliberately loose (the fast call must beat the slow
    /// one's own delay); the assertion is about overlap, not about latency.
    #[test]
    fn a_slow_endpoint_does_not_hold_the_other_callers() {
        const SLOW_CHAIN: u32 = 424_242;
        const FAST_CHAIN: u32 = 424_243;
        const DELAY: Duration = Duration::from_millis(1_500);

        storage::tests::with_temp_state("pool-concurrent", || {
            let slow = fake_rpc(SLOW_CHAIN, DELAY);
            let fast = fake_rpc(FAST_CHAIN, Duration::ZERO);
            if storage::write_value(
                storage::KEY_CUSTOM_NETWORKS,
                json!([
                    { "chainId": SLOW_CHAIN, "rpcURL": format!("http://127.0.0.1:{slow}") },
                    { "chainId": FAST_CHAIN, "rpcURL": format!("http://127.0.0.1:{fast}") },
                ]),
            )
            .is_err()
            {
                unreachable!("could not seed the two networks");
            }

            let began = Instant::now();
            let slow_call =
                std::thread::spawn(move || call(SLOW_CHAIN, "eth_blockNumber", json!([])));
            // Long enough that the slow call is certainly posting, short
            // enough that it is certainly not finished.
            std::thread::sleep(Duration::from_millis(300));

            let fast_started = Instant::now();
            let answer = call(FAST_CHAIN, "eth_blockNumber", json!([]));
            let waited = fast_started.elapsed();
            let at = began.elapsed();

            assert!(
                answer.is_ok(),
                "the fast endpoint did not answer: {answer:?}"
            );
            assert!(
                at < DELAY,
                "the fast call finished at {at:?}, after the slow endpoint's \
                 own {DELAY:?} — the pool is still a queue of one"
            );
            println!("  fast call answered in {waited:?}, {at:?} into a {DELAY:?} wait");

            let slow_answer = slow_call
                .join()
                .unwrap_or_else(|_| unreachable!("the slow caller panicked"));
            assert!(slow_answer.is_ok(), "the slow endpoint never answered");
        });
    }

    /// A JSON-RPC server on a loopback port that answers after `delay`.
    /// Returns the port. It lives as long as the process; the test binary
    /// exiting is what stops it.
    fn fake_rpc(chain_id: u32, delay: Duration) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|error| unreachable!("no loopback port: {error}"));
        let port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_else(|error| unreachable!("no port: {error}"));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || serve_one(stream, chain_id, delay));
            }
        });
        port
    }

    /// The body of one HTTP request read off `stream`.
    fn read_request(stream: &std::net::TcpStream) -> String {
        use std::io::{BufRead as _, Read as _};

        let mut reader = std::io::BufReader::new(
            stream
                .try_clone()
                .unwrap_or_else(|error| unreachable!("clone: {error}")),
        );
        let mut length = 0usize;
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            line.clear();
        }
        let mut body = vec![0u8; length];
        let _ = reader.read_exact(&mut body);
        String::from_utf8_lossy(&body).into_owned()
    }

    fn serve_one(mut stream: std::net::TcpStream, chain_id: u32, delay: Duration) {
        use std::io::Write as _;

        let body = read_request(&stream);
        std::thread::sleep(delay);
        // A custom network is verified before it is trusted, so the chain id
        // this server reports has to be the one the pool asked for.
        let result = if body.contains("eth_chainId") {
            format!("0x{chain_id:x}")
        } else {
            "0x1".to_owned()
        };
        let payload = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"{result}\"}}");
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }

    /// A loopback node for the hedging tests (083 H6). It notes when each
    /// request arrived, waits `delay`, then writes `response` — or, given
    /// `None`, closes the connection unanswered: a node that swallows the
    /// read, as the black-holed one in W13 did. Returns its URL and its log.
    fn scripted_node(
        delay: Duration,
        response: Option<String>,
    ) -> (String, std::sync::Arc<Mutex<Vec<Instant>>>) {
        use std::io::Write as _;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|error| unreachable!("no loopback port: {error}"));
        let port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_else(|error| unreachable!("no port: {error}"));
        let log = std::sync::Arc::new(Mutex::new(Vec::new()));
        let seen = std::sync::Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (seen, response) = (std::sync::Arc::clone(&seen), response.clone());
                std::thread::spawn(move || {
                    let _ = read_request(&stream);
                    if let Ok(mut seen) = seen.lock() {
                        seen.push(Instant::now());
                    }
                    std::thread::sleep(delay);
                    if let Some(response) = response {
                        let _ = (&stream).write_all(response.as_bytes());
                    }
                });
            }
        });
        (format!("http://127.0.0.1:{port}"), log)
    }

    fn http(status: &str, payload: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        )
    }

    const ANSWER: &str = r#"{"jsonrpc":"2.0","id":1,"result":"0x2a"}"#;

    fn arrivals(log: &Mutex<Vec<Instant>>) -> Vec<Instant> {
        log.lock().map(|seen| seen.clone()).unwrap_or_default()
    }

    /// One chain whose pool asks `first` (its per-network override) and then
    /// `second` (its own endpoint), in that order.
    fn seed_two(chain_id: u32, first: &str, second: &str) {
        let seeded = storage::write_value(
            storage::KEY_NETWORK_CONFIG,
            json!([{ "chainId": chain_id, "rpcURL": first }]),
        )
        .and_then(|()| {
            storage::write_value(
                storage::KEY_CUSTOM_NETWORKS,
                json!([{ "chainId": chain_id, "rpcURL": second }]),
            )
        });
        if seeded.is_err() {
            unreachable!("could not seed the chain");
        }
        let (rpc, _) = collect_endpoints(chain_id);
        let urls: Vec<&str> = rpc.iter().map(|endpoint| endpoint.url.as_str()).collect();
        assert_eq!(urls, [first, second]);
    }

    fn open_calls() -> usize {
        let (reply, answer) = channel();
        let sent = sender()
            .lock()
            .ok()
            .is_some_and(|tx| tx.send(Message::Ask(Request::Open { reply })).is_ok());
        assert!(sent, "the pool thread is gone");
        answer.recv().unwrap_or(usize::MAX)
    }

    /// 083 H6, the case W13 measured: the chain's first node swallows reads.
    /// A read used to wait out the core's whole 8 s read timeout before the
    /// next node was asked; now the next node is asked after `HEDGE_AFTER`,
    /// and its answer is the caller's.
    ///
    /// Asked ONCE: when the core, after the first node finally fails, reaches
    /// the second, it takes the hedge's answer instead of posting again — and
    /// the call then closes, leaving nothing behind. The core was told the
    /// first node's real failure, so the next read starts at the second node
    /// and waits for no hedge at all.
    #[test]
    fn a_black_holed_node_is_answered_by_the_next_after_the_hedge_delay() {
        const CHAIN: u32 = 424_244;
        const HOLD: Duration = Duration::from_secs(3);

        storage::tests::with_temp_state("pool-hedge", || {
            let (dead, dead_log) = scripted_node(HOLD, None);
            let (live, live_log) = scripted_node(Duration::ZERO, Some(http("200 OK", ANSWER)));
            seed_two(CHAIN, &dead, &live);

            let began = Instant::now();
            let answer = call(CHAIN, "eth_blockNumber", json!([]));
            let waited = began.elapsed();
            assert_eq!(
                answer.ok().and_then(|body| body.get("result").cloned()),
                Some(json!("0x2a"))
            );
            assert!(
                waited >= HEDGE_AFTER,
                "answered before any hedge: {waited:?}"
            );
            assert!(
                waited < HEDGE_AFTER + Duration::from_secs(1),
                "the hedge came late: {waited:?}"
            );
            println!("  answered in {waited:?} with the first node silent");

            // Past the first node's own failure, and the core's last word.
            std::thread::sleep(HOLD.saturating_sub(waited) + Duration::from_millis(700));
            assert_eq!(arrivals(&dead_log).len(), 1);
            assert_eq!(
                arrivals(&live_log).len(),
                1,
                "the second node was asked twice for one read"
            );
            assert_eq!(open_calls(), 0, "an answered read was left open");

            let again = Instant::now();
            assert!(call(CHAIN, "eth_blockNumber", json!([])).is_ok());
            assert!(
                again.elapsed() < HEDGE_AFTER,
                "the core never learned the first node failed"
            );
            assert_eq!(arrivals(&dead_log).len(), 1);
        });
    }

    /// 083 H6: a write is never sent twice. The first node is slow — well
    /// past the hedge delay — and the second never hears of it.
    #[test]
    fn a_write_is_never_hedged() {
        const CHAIN: u32 = 424_245;
        const SLOW: Duration = Duration::from_millis(2_500);

        storage::tests::with_temp_state("pool-hedge-write", || {
            let (first, first_log) = scripted_node(SLOW, Some(http("200 OK", ANSWER)));
            let (second, second_log) = scripted_node(Duration::ZERO, Some(http("200 OK", ANSWER)));
            seed_two(CHAIN, &first, &second);

            let began = Instant::now();
            let answer = call(CHAIN, "eth_sendRawTransaction", json!(["0x02f8"]));
            assert!(answer.is_ok(), "{answer:?}");
            assert!(began.elapsed() >= SLOW, "answered by another node");
            assert_eq!(arrivals(&first_log).len(), 1);
            assert!(arrivals(&second_log).is_empty(), "a write was hedged");

            // Nor is anything bound for the bundler, whatever it is called.
            assert!(Hedge::for_call(CHAIN, RpcKind::Rpc, "eth_sendRawTransaction").is_none());
            assert!(
                Hedge::for_call(CHAIN, RpcKind::Bundler, "eth_getUserOperationReceipt").is_none()
            );
            assert!(Hedge::for_call(CHAIN, RpcKind::Rpc, "eth_call").is_some());
        });
    }

    /// 083 H6: when every node fails, the caller hears the pool's failure,
    /// classified from the nodes in the core's order as it always was — never
    /// the hedge's own failure in its place. The first node is rate-limited
    /// (429, slowly) and the second fails at once (502): the verdict is the
    /// first node's "rate-limited", the transient case the balance screen
    /// keeps quiet about (invariant ④), not the second node's plain failure.
    #[test]
    fn when_both_nodes_fail_the_first_nodes_failure_is_reported() {
        const CHAIN: u32 = 424_246;
        const SLOW: Duration = Duration::from_millis(2_000);

        storage::tests::with_temp_state("pool-hedge-fail", || {
            let (first, _) = scripted_node(SLOW, Some(http("429 Too Many Requests", "")));
            let (second, second_log) =
                scripted_node(Duration::ZERO, Some(http("502 Bad Gateway", "")));
            seed_two(CHAIN, &first, &second);

            let began = Instant::now();
            let answer = call(CHAIN, "eth_blockNumber", json!([]));
            assert_eq!(answer, Err(PoolError::Failed { rate_limited: true }));

            // The hedge went out while the first node was still silent, and
            // the second node was asked once per pass: the first pass's visit
            // was the hedge's answer, not a second request.
            let second = arrivals(&second_log);
            assert!(
                second
                    .first()
                    .is_some_and(|at| at.duration_since(began) < SLOW),
                "no hedge was sent"
            );
            assert_eq!(
                second.len(),
                vela_core::app::rpc_pool::MAX_RPC_ATTEMPTS as usize
            );
            assert_eq!(open_calls(), 0);
        });
    }

    /// 083 H6, the bound: a read hedges to each endpoint at most once, with at
    /// most one hedge unanswered at a time, never to a banned endpoint, and
    /// stops when the pool is spent.
    #[test]
    fn a_read_hedges_each_endpoint_once_and_one_at_a_time() {
        let seeds: Vec<RpcEndpointSeed> = ["https://a", "https://b", "https://c", "https://d"]
            .into_iter()
            .map(|url| RpcEndpointSeed {
                url: url.to_owned(),
                source: RpcSource::Public,
            })
            .collect();
        let now = now_ms();
        let banned = [RpcBanEntry {
            url: "https://c".to_owned(),
            banned_at_ms: now - 1_000.0,
            permanent: false,
        }];
        let Some(mut hedge) = Hedge::for_call(1, RpcKind::Rpc, "eth_call") else {
            unreachable!("eth_call is a read");
        };
        assert_eq!(hedge.due(), None, "nothing to hedge before the core posts");

        // The core posts to the first endpoint.
        let sent_at = Instant::now();
        hedge.asked.insert("https://a".to_owned());
        hedge.current = Some(("https://a".to_owned(), sent_at));
        assert_eq!(hedge.due(), Some(sent_at + HEDGE_AFTER));

        // Each hedge, as `hedge_due_reads` sends it, then lands failing.
        let mut targets = Vec::new();
        while hedge.due().is_some() {
            let Some(url) = hedge.target(&seeds, &banned, now) else {
                hedge.spent = true;
                continue;
            };
            hedge.asked.insert(url.clone());
            hedge.flying.insert(url.clone(), (sent_at, None));
            assert_eq!(hedge.due(), None, "two hedges unanswered at once");
            hedge.flying.remove(&url);
            targets.push(url);
        }
        assert_eq!(targets, ["https://b", "https://d"]);
    }

    /// The pool, against the real network, through the real routing.
    ///
    /// This is SC-002's evidence and it is deliberately a READ of the golden
    /// Safe: the balance it returns is checkable by hand, and chain 100's
    /// built-in endpoint is the one that answers this client with 403 — so the
    /// pool has to fail over to reach it. A single-endpoint client cannot.
    #[test]
    #[ignore = "hits the real Gnosis pool"]
    fn the_pool_reads_the_golden_safe_by_failing_over() {
        storage::tests::with_temp_state("pool-live", || {
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

            let balance = call(100, "eth_getBalance", json!([GOLDEN, "latest"]))
                .unwrap_or_else(|error| unreachable!("the pool could not read: {error:?}"));
            let hex = balance
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or_else(|| unreachable!("no result member: {balance}"));
            let wei = u128::from_str_radix(hex.trim_start_matches("0x"), 16)
                .unwrap_or_else(|_| unreachable!("not a quantity: {hex}"));
            #[allow(clippy::cast_precision_loss, reason = "display only")]
            let xdai = wei as f64 / 1e18;
            println!("  golden Safe: {xdai} xDAI via the pool");
            assert!(
                wei > 0,
                "the golden Safe is funded; a zero means we read nothing"
            );

            // A second call proves the session is shared: whatever the first
            // call learned — which endpoint answered, which one is banned — is
            // still known, and the answer agrees.
            let again = call(100, "eth_getBalance", json!([GOLDEN, "latest"]))
                .unwrap_or_else(|error| unreachable!("second read failed: {error:?}"));
            assert_eq!(
                again.get("result"),
                balance.get("result"),
                "two reads of one address disagreed"
            );
            println!("  second read agreed — one session, shared state");
        });
    }
}
