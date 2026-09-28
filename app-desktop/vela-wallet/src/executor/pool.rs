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

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::net_health::{NetEdge, NetHealth, net_health_step};
use vela_core::app::network_admin::{
    BUILTIN_CHAINS, NetProviderId, PROVIDER_ORDER, build_provider_rpc_url,
};
use vela_core::app::rpc_pool::{
    Event, RpcBanEntry, RpcCallVerdict, RpcEndpointSeed, RpcKind, RpcOperation, RpcShellResult,
    RpcSource, RpcTransportOutcome,
};

use crate::core_host::CoreHost;
use crate::diag::{host_of, vlog};
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

/// What a caller asked for and where to send the answer.
enum Request {
    Call {
        chain_id: u32,
        kind: RpcKind,
        method: String,
        params: Value,
        reply: Sender<Routed>,
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
    /// The chains whose first pass reached no endpoint (spec 082 RF1).
    Unreached { reply: Sender<Vec<u32>> },
}

/// Why a routed call produced no answer.
#[derive(Debug, Clone, PartialEq)]
pub enum PoolError {
    /// Every endpoint failed every pass. `rate_limited` is the self-healing
    /// transient case (invariant ④) and is worth showing differently.
    Failed { rate_limited: bool },
    /// `eth_getLogs` hit a range cap; the caller splits and retries.
    /// `error_json` is the JSON-RPC `error` member the endpoint answered, as
    /// it came (spec 082 T180): the tracker's find-event hands it to the core,
    /// which alone decides what a range error is and halves its window —
    /// dropping it here left the same too-wide window asked forever.
    RangeCap {
        max_span: f64,
        error_json: Option<String>,
    },
    /// The pool thread is gone. Only reachable if it panicked.
    Unavailable,
}

/// A routed call's answer, and whether any POST of it may have reached an
/// endpoint that acted on it while its reply was lost (spec 082 RA1: the OR
/// of `rpc_pool::may_have_delivered` over every POST). Only the submit reads
/// the second half; everything else takes [`Routed::answer`].
#[derive(Debug, Clone, PartialEq)]
pub struct Routed {
    pub answer: Result<Value, PoolError>,
    pub maybe_delivered: bool,
}

impl Routed {
    fn unavailable() -> Self {
        Self {
            answer: Err(PoolError::Unavailable),
            maybe_delivered: false,
        }
    }
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

/// [`bundler_call`] with the delivery half of the verdict — the submit's
/// entry point (spec 082 RA1): `submit_step` needs to know whether a lost
/// reply may hide an accepted operation.
pub fn bundler_routed(chain_id: u32, method: &str, params: Value) -> Routed {
    route(chain_id, RpcKind::Bundler, method, params)
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
    route_within(chain_id, RpcKind::Bundler, method, params, budget)
}

/// [`call`], waiting at most `budget` — for a best-effort read that must not
/// hold its caller for a whole sweep (the submit's head read, spec 082).
pub fn call_within(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    route_within(chain_id, RpcKind::Rpc, method, params, budget)
}

fn route_within(
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
        Ok(routed) => routed.answer,
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

/// The chains whose first pass of a call reached no endpoint at all, with no
/// rate limit in sight (`RpcPoolView.unreached_chains`, spec 082 RF1): the
/// chain notice's early half, a pass sooner than [`failed_chains`]. The home
/// RPC banner does not read it. **Blocks** on the pool thread, briefly.
pub fn unreached_chains() -> Vec<u32> {
    let (reply, answer) = channel();
    let sent = sender()
        .lock()
        .ok()
        .is_some_and(|tx| tx.send(Message::Ask(Request::Unreached { reply })).is_ok());
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
    route(chain_id, kind, method, params).answer
}

fn route(chain_id: u32, kind: RpcKind, method: &str, params: Value) -> Routed {
    let (reply, answer) = channel();
    {
        let Ok(tx) = sender().lock() else {
            return Routed::unavailable();
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
            return Routed::unavailable();
        }
    }
    answer.recv().unwrap_or_else(|_| Routed::unavailable())
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
    reply: Sender<Routed>,
    /// The last body received, per URL. `Conclude { Respond { url } }` names
    /// which one the core accepted — the core never sees a body itself.
    bodies: HashMap<String, Value>,
    /// For the log lines and the network-health count only.
    chain_id: u32,
    kind: RpcKind,
    method: String,
    started: Instant,
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

    while let Ok(message) = rx.recv() {
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
                let pending = host.resolve(id, result);
                drain(&mut host, pending, &mut inflight, &mut queries, workers);
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
                        reply,
                        bodies: HashMap::new(),
                        chain_id,
                        kind,
                        method: method.clone(),
                        started: Instant::now(),
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
            Request::Unreached { reply } => {
                let _ = reply.send(host.view().unreached_chains);
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
        if offload(next.id, &next.operation, inflight, workers) {
            continue;
        }
        let result = perform(&next.operation, inflight, queries);
        queue.extend(host.resolve(next.id, result));
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
    use std::sync::atomic::Ordering;

    let work: Box<dyn FnOnce() -> Message + Send> = match operation {
        RpcOperation::JsonRpcPost {
            call_id,
            url,
            method,
            x_rpc_url,
            timeout_ms,
        } => {
            // The params are read here, on the thread that owns `inflight`.
            let (params, chain_id) = inflight.get(call_id).map_or_else(
                || (Value::Array(Vec::new()), 0),
                |call| (call.params.clone(), call.chain_id),
            );
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
                log_post(chain_id, &url, &method, &outcome);
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
            let (params, chain_id) = inflight.get(call_id).map_or_else(
                || (Value::Array(Vec::new()), 0),
                |call| (call.params.clone(), call.chain_id),
            );
            let started = Instant::now();
            let (outcome, body) = post(url, method, &params, x_rpc_url.as_deref(), *timeout_ms);
            log_post(chain_id, url, method, &outcome);
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
            if let Some(call) = inflight.remove(call_id) {
                let routed = routed_of(verdict, &call.bodies);
                note_conclusion(&call, verdict);
                // A caller that gave up is not an error: the receiver is simply
                // gone, and the pool has nothing to be sad about.
                let _ = call.reply.send(routed);
            }
            RpcShellResult::Concluded
        }
    }
}

/// The core's verdict as the caller's answer: the body it accepted, or why
/// there is none — with the delivery bit the submit reads (spec 082 RA1).
fn routed_of(verdict: &RpcCallVerdict, bodies: &HashMap<String, Value>) -> Routed {
    match verdict {
        RpcCallVerdict::Respond {
            url,
            maybe_delivered,
        } => Routed {
            answer: bodies.get(url).cloned().ok_or(PoolError::Failed {
                rate_limited: false,
            }),
            maybe_delivered: *maybe_delivered,
        },
        RpcCallVerdict::RangeCap { url, max_span } => Routed {
            answer: Err(PoolError::RangeCap {
                max_span: *max_span,
                error_json: bodies
                    .get(url)
                    .and_then(|body| body.get("error"))
                    .map(Value::to_string),
            }),
            maybe_delivered: false,
        },
        RpcCallVerdict::Failed {
            rate_limited,
            maybe_delivered,
        } => Routed {
            answer: Err(PoolError::Failed {
                rate_limited: *rate_limited,
            }),
            maybe_delivered: *maybe_delivered,
        },
        // Not answers to a routed call; a caller waiting on one of these asked
        // the wrong question.
        RpcCallVerdict::BundlerBase { .. } | RpcCallVerdict::BestRpcUrl { .. } => Routed {
            answer: Err(PoolError::Failed {
                rate_limited: false,
            }),
            maybe_delivered: false,
        },
    }
}

// ---------------------------------------------------------------------------
// Is the network there (spec 082 RE3, T069)
// ---------------------------------------------------------------------------

/// The count the pool keeps from its own conclusions — the one place every
/// chain read of the app passes through. `net_health_step` is the core's.
static HEALTH: Mutex<NetHealth> = Mutex::new(NetHealth {
    misses: 0,
    online: true,
    sources: Vec::new(),
    unsourced: 0,
    last_reach_ms: None,
    run_started_ms: None,
});
/// Bumped on every "came back" edge. The browser reads it on its poll and
/// retries a failed page (and the balances are invalidated right here), so no
/// callback crosses from this thread into the window's.
static CAME_BACK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many times the network has come back since launch.
pub fn came_back_count() -> u64 {
    CAME_BACK.load(std::sync::atomic::Ordering::SeqCst)
}

/// What one routed chain read says about the network (the rule in
/// `net_health`'s module note): any answer reached a server; every endpoint
/// swept with none answering — timeouts included, a hanging proxy node only
/// ever times out — is a miss; a throttled sweep says nothing either way.
/// Only chain reads count: the relay being down is not the network.
fn reached_of(verdict: &RpcCallVerdict) -> Option<bool> {
    match verdict {
        RpcCallVerdict::Respond { .. } | RpcCallVerdict::RangeCap { .. } => Some(true),
        RpcCallVerdict::Failed {
            rate_limited: false,
            ..
        } => Some(false),
        RpcCallVerdict::Failed {
            rate_limited: true, ..
        }
        | RpcCallVerdict::BundlerBase { .. }
        | RpcCallVerdict::BestRpcUrl { .. } => None,
    }
}

/// Feed one conclusion into the count. `Some(edge)` when it crossed one.
fn feed_health(reached: bool) -> Option<NetEdge> {
    let Ok(mut health) = HEALTH.lock() else {
        return None;
    };
    let (next, edge) = net_health_step(health.clone(), reached, None, now_ms());
    *health = next;
    edge
}

/// The call is over: log a give-up, and count it towards the network's health.
fn note_conclusion(call: &InFlight, verdict: &RpcCallVerdict) {
    if let RpcCallVerdict::Failed { rate_limited, .. } = verdict {
        vlog!(
            "rpc",
            "chain={} method={} gave up after {} ms{}",
            call.chain_id,
            call.method,
            call.started.elapsed().as_millis(),
            if *rate_limited { " (rate limited)" } else { "" }
        );
    }
    if call.kind != RpcKind::Rpc {
        return;
    }
    let Some(reached) = reached_of(verdict) else {
        return;
    };
    match feed_health(reached) {
        Some(NetEdge::WentOffline) => vlog!("net", "offline (chain reads stopped answering)"),
        Some(NetEdge::CameBack) => {
            vlog!("net", "came back");
            CAME_BACK.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // RE3: a balance read on the way back, not at the next 10 min pass.
            crate::executor::balance_dashboard::invalidate();
        }
        None => {}
    }
}

/// One POST that did not get an answer, as a log line: which chain, which
/// host (never the URL — a provider's key lives in its path), which method
/// and what happened. Answers are not logged.
fn log_post(chain_id: u32, url: &str, method: &str, outcome: &RpcTransportOutcome) {
    let outcome = match outcome {
        RpcTransportOutcome::Response { .. } => return,
        RpcTransportOutcome::HttpError { status } => format!("http {status}"),
        RpcTransportOutcome::NonJson => "non-json".to_owned(),
        RpcTransportOutcome::Timeout => "timeout".to_owned(),
        RpcTransportOutcome::Network => "network".to_owned(),
        RpcTransportOutcome::NotConnected => "not connected".to_owned(),
    };
    vlog!(
        "rpc",
        "chain={chain_id} host={} method={method} outcome={outcome}",
        host_of(url)
    );
}

/// A failed request in the pool's five-plus-one outcomes (spec 082 RA1).
///
/// The one question that matters for money: could the request have reached
/// the endpoint? `NotConnected` only when it provably did not — the name did
/// not resolve, nothing accepted the connection, the TLS handshake (which
/// precedes the request's first byte) was refused, or the proxy would not
/// open the tunnel. Anything that may have happened after the request was
/// written is `Timeout` or `Network`, which the core reads as "may have been
/// delivered". A timeout counts as not connected only when ureq names the
/// resolve or connect phase; the global timeout it reports otherwise is the
/// safe side.
pub fn transport_outcome_of(error: &ureq::Error) -> RpcTransportOutcome {
    use std::io::ErrorKind;
    use ureq::Timeout;
    match error {
        ureq::Error::StatusCode(status) => RpcTransportOutcome::HttpError { status: *status },
        ureq::Error::Timeout(Timeout::Resolve | Timeout::Connect) => {
            RpcTransportOutcome::NotConnected
        }
        ureq::Error::Timeout(_) => RpcTransportOutcome::Timeout,
        // The proxy's CONNECT was refused (502), closed with no reply, or
        // answered something else: the tunnel never opened, so the request
        // never left.
        ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::InvalidProxyUrl
        | ureq::Error::Http(_)
        | ureq::Error::Tls(_)
        | ureq::Error::TlsRequired
        | ureq::Error::RequireHttpsOnly(_) => RpcTransportOutcome::NotConnected,
        ureq::Error::Rustls(tls) if is_handshake_refusal(tls) => RpcTransportOutcome::NotConnected,
        ureq::Error::Io(io) => {
            let refused_here = matches!(
                io.kind(),
                ErrorKind::ConnectionRefused
                    | ErrorKind::AddrNotAvailable
                    | ErrorKind::HostUnreachable
                    | ErrorKind::NetworkUnreachable
                    | ErrorKind::NetworkDown
            );
            let tls_refused = io
                .get_ref()
                .and_then(|inner| inner.downcast_ref::<rustls::Error>())
                .is_some_and(is_handshake_refusal);
            if refused_here || tls_refused || resolver_said_no(&io.to_string()) {
                RpcTransportOutcome::NotConnected
            } else {
                RpcTransportOutcome::Network
            }
        }
        _ => RpcTransportOutcome::Network,
    }
}

/// A TLS failure that can only happen before the handshake completes — so
/// before the request's first byte: the certificate (a server sends it in the
/// handshake and nowhere else), a peer with nothing in common, or an alert
/// that only a handshake raises.
fn is_handshake_refusal(error: &rustls::Error) -> bool {
    use rustls::AlertDescription as A;
    match error {
        rustls::Error::InvalidCertificate(_)
        | rustls::Error::NoCertificatesPresented
        | rustls::Error::PeerIncompatible(_)
        | rustls::Error::HandshakeNotComplete
        | rustls::Error::UnsupportedNameType => true,
        rustls::Error::AlertReceived(alert) => matches!(
            alert,
            A::HandshakeFailure
                | A::BadCertificate
                | A::UnsupportedCertificate
                | A::CertificateRevoked
                | A::CertificateExpired
                | A::CertificateUnknown
                | A::UnknownCA
                | A::ProtocolVersion
                | A::InsufficientSecurity
        ),
        _ => false,
    }
}

/// A resolver failure arrives as an uncategorised io error carrying libc's
/// sentence; these are the three desktops' words for "the name did not look
/// up" (the same fragments `proxy.rs` reads).
fn resolver_said_no(message: &str) -> bool {
    message.contains("failed to lookup address information")
        || message.contains("nodename nor servname")
        || message.contains("Name or service not known")
        || message.contains("No such host is known")
        || message.contains("No address associated with hostname")
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
    // Per URL, over the system's routes for it (spec 082 RD2): loopback is
    // direct, the next route is tried only when a proxy itself could not be
    // reached, and a dead proxy is named in the log rather than routed around.
    let timeout = Duration::from_millis(u64::from(timeout_ms));
    let mut response = match proxy::with_routes(url, timeout, |agent| {
        // Spec 081 FR-007: the wallet no longer tells the relay which RPC endpoint it prefers. That header carried the user's first-choice URL, which can contain a provider API key — and the relay never read this name anyway (it reads `x-vela-rpc-url`), so nothing depended on it.
        let _ = x_rpc_url;
        agent
            .post(url)
            .header("content-type", "application/json")
            .send_json(&payload)
    }) {
        Ok(response) => response,
        Err(failure) => return (transport_outcome_of(&failure.error), None),
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

    /// A one-connection HTTP proxy on loopback that reads the CONNECT and
    /// answers it with `reply` — or closes with no reply at all when `reply`
    /// is empty. Its port.
    fn proxy_answering(reply: &'static [u8]) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| unreachable!("loopback: {e}"));
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
        std::thread::spawn(move || {
            use std::io::{Read as _, Write as _};
            if let Some(Ok(mut stream)) = listener.incoming().next() {
                // The whole CONNECT, so closing sends a FIN, not a reset for
                // bytes nobody read.
                let mut seen = Vec::new();
                let mut buf = [0u8; 512];
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => seen.extend_from_slice(&buf[..n]),
                    }
                }
                if !reply.is_empty() {
                    let _ = stream.write_all(reply);
                    let _ = stream.flush();
                }
                let _ = stream.shutdown(std::net::Shutdown::Write);
                // Hold the socket until the client lets go.
                while matches!(stream.read(&mut buf), Ok(n) if n > 0) {}
            }
        });
        port
    }

    /// The submit's POST through a proxy at `port`, and the error it ends in.
    fn post_through(port: u16) -> ureq::Error {
        let proxy = ureq::Proxy::new(&format!("http://127.0.0.1:{port}"))
            .unwrap_or_else(|e| unreachable!("proxy: {e}"));
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(Some(proxy))
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        match agent
            .post("https://relay.example/100")
            .send_json(json!({"jsonrpc": "2.0", "method": "eth_sendUserOperation"}))
        {
            Ok(_) => unreachable!("nothing answers the tunnel"),
            Err(error) => error,
        }
    }

    /// Spec 082 T050: the exact ureq 3.4 error for a proxy that would not
    /// open the tunnel — a 502 to the CONNECT — is pinned here. The request
    /// itself is written only through an open tunnel, so nothing left the
    /// machine: `NotConnected`, the one outcome that lets a submit say "not
    /// sent".
    #[test]
    fn a_refused_connect_is_not_connected() {
        let error = post_through(proxy_answering(
            b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\n\r\n",
        ));
        assert!(
            matches!(&error, ureq::Error::ConnectProxyFailed(reason) if reason.contains("502")),
            "ureq 3.4 names a refused tunnel ConnectProxyFailed: {error:?}"
        );
        assert_eq!(
            transport_outcome_of(&error),
            RpcTransportOutcome::NotConnected
        );
    }

    /// A proxy that reads the CONNECT and closes with no answer (chaos
    /// `drop`): the tunnel never opened either, and ureq 3.4 says so with the
    /// same variant — pinned, so an upgrade that renames it fails here rather
    /// than turning a refusal into "may have been sent".
    #[test]
    fn a_connect_closed_with_no_reply_is_not_connected() {
        let error = post_through(proxy_answering(b""));
        assert!(
            matches!(&error, ureq::Error::ConnectProxyFailed(reason) if reason.contains("did not respond")),
            "{error:?}"
        );
        assert_eq!(
            transport_outcome_of(&error),
            RpcTransportOutcome::NotConnected
        );
    }

    /// The rest of the "nothing left the machine" list (contract §2), and the
    /// failures that may have happened after the request was written.
    #[test]
    fn transport_errors_say_whether_the_request_could_have_left() {
        use std::io::{Error, ErrorKind};
        let not_connected = [
            ureq::Error::HostNotFound,
            ureq::Error::ConnectionFailed,
            ureq::Error::Timeout(ureq::Timeout::Resolve),
            ureq::Error::Timeout(ureq::Timeout::Connect),
            ureq::Error::ConnectProxyFailed("proxy server responded 502/Bad Gateway".to_owned()),
            ureq::Error::Io(Error::from(ErrorKind::ConnectionRefused)),
            ureq::Error::Io(Error::from(ErrorKind::AddrNotAvailable)),
            ureq::Error::Io(Error::from(ErrorKind::HostUnreachable)),
            ureq::Error::Io(Error::from(ErrorKind::NetworkUnreachable)),
            ureq::Error::Io(Error::other(
                "failed to lookup address information: nodename nor servname provided",
            )),
            ureq::Error::Tls("handshake"),
            ureq::Error::Rustls(rustls::Error::InvalidCertificate(
                rustls::CertificateError::Expired,
            )),
            ureq::Error::Rustls(rustls::Error::AlertReceived(
                rustls::AlertDescription::HandshakeFailure,
            )),
        ];
        for error in &not_connected {
            assert_eq!(
                transport_outcome_of(error),
                RpcTransportOutcome::NotConnected,
                "{error:?}"
            );
        }
        // After the request may have been written: the safe side.
        assert_eq!(
            transport_outcome_of(&ureq::Error::Timeout(ureq::Timeout::Global)),
            RpcTransportOutcome::Timeout
        );
        assert_eq!(
            transport_outcome_of(&ureq::Error::Timeout(ureq::Timeout::RecvResponse)),
            RpcTransportOutcome::Timeout
        );
        for error in [
            ureq::Error::Io(Error::from(ErrorKind::ConnectionReset)),
            ureq::Error::Io(Error::from(ErrorKind::UnexpectedEof)),
            ureq::Error::Io(Error::from(ErrorKind::BrokenPipe)),
            ureq::Error::Rustls(rustls::Error::AlertReceived(
                rustls::AlertDescription::CloseNotify,
            )),
        ] {
            assert_eq!(
                transport_outcome_of(&error),
                RpcTransportOutcome::Network,
                "{error:?}"
            );
        }
        assert_eq!(
            transport_outcome_of(&ureq::Error::StatusCode(503)),
            RpcTransportOutcome::HttpError { status: 503 }
        );
    }

    /// The pool's verdict carries the delivery bit, and a range limit keeps
    /// the endpoint's own error member (T180) for the tracker's find-event.
    #[test]
    fn the_verdict_keeps_the_delivery_bit_and_the_range_error() {
        let url = "https://rpc.example/".to_owned();
        let words = json!({"code": -32005, "message": "query exceeds max block range 1000"});
        let bodies: HashMap<String, Value> =
            std::iter::once((url.clone(), json!({ "error": words.clone() }))).collect();
        let capped = routed_of(
            &RpcCallVerdict::RangeCap {
                url: url.clone(),
                max_span: 1000.0,
            },
            &bodies,
        );
        assert_eq!(
            capped.answer,
            Err(PoolError::RangeCap {
                max_span: 1000.0,
                error_json: Some(words.to_string()),
            })
        );
        let lost = routed_of(
            &RpcCallVerdict::Failed {
                rate_limited: false,
                maybe_delivered: true,
            },
            &bodies,
        );
        assert!(lost.maybe_delivered);
        assert!(lost.answer.is_err());
    }

    /// Spec 082 T069: every unthrottled sweep with no answer is a miss —
    /// timeouts included — and any answer a reach; a throttled one says
    /// nothing.
    #[test]
    fn a_sweep_with_no_answer_is_a_miss_and_a_throttled_one_is_nothing() {
        assert_eq!(
            reached_of(&RpcCallVerdict::Failed {
                rate_limited: false,
                maybe_delivered: true,
            }),
            Some(false)
        );
        assert_eq!(
            reached_of(&RpcCallVerdict::Failed {
                rate_limited: true,
                maybe_delivered: false,
            }),
            None
        );
        assert_eq!(
            reached_of(&RpcCallVerdict::Respond {
                url: String::new(),
                maybe_delivered: false,
            }),
            Some(true)
        );
    }

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

    fn serve_one(mut stream: std::net::TcpStream, chain_id: u32, delay: Duration) {
        use std::io::{BufRead as _, Read as _, Write as _};

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
        let body = String::from_utf8_lossy(&body).into_owned();

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
