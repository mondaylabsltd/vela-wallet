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
//! asks the endpoint the core will ask next — named by the core's own view,
//! never by a tier walk here — the caller may take that answer only when the
//! core says it is one every node gives alike, and the core is still told
//! every outcome in its own order. See [`Hedge`].

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::net_health::{NetEdge, NetHealth, net_health_step};
use vela_core::app::network_admin::{
    BUILTIN_CHAINS, NetProviderId, PROVIDER_ORDER, build_provider_rpc_url,
};
use vela_core::app::rpc_pool::{
    Event, HEDGE_AFTER_MS, RPC_READ_TIMEOUT_MS, RpcBanEntry, RpcCallVerdict, RpcEndpointSeed,
    RpcKind, RpcOperation, RpcPoolView, RpcShellResult, RpcSource, RpcTransportOutcome,
    early_verdict, is_ban_active, is_hedged_read,
};

use crate::core_host::CoreHost;
use crate::diag::{host_of, vlog};
use crate::executor::{proxy, storage};

/// How long a read waits on one endpoint before the same read also goes to
/// the endpoint the core asks next (spec 083 D3b, hand-off H6). The delay,
/// which reads may be hedged ([`is_hedged_read`]), where a hedge goes
/// ([`RpcPoolView::pending_urls`]) and which early answer a caller may take
/// ([`early_verdict`]) are the core's, so another shell can hedge alike;
/// this file only sends the post and holds the reply.
const HEDGE_AFTER: Duration = Duration::from_millis(HEDGE_AFTER_MS as u64);

/// After a hedge found no free worker, how long before the read tries again.
const HEDGE_RETRY: Duration = Duration::from_millis(250);

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
    /// Which RPC the pool would name to the relay for this chain (spec 098 §5).
    BestRpcUrl {
        chain_id: u32,
        reply: Sender<Option<String>>,
    },
    /// The chains whose last failure was the providers' rate limit.
    RateLimited { reply: Sender<Vec<u32>> },
    /// The chains whose whole RPC pool failed on the last attempt.
    Failed { reply: Sender<Vec<u32>> },
    /// The chains whose first pass reached no endpoint (spec 082 RF1).
    Unreached { reply: Sender<Vec<u32>> },
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
/// held past it by one call's timeouts and retries. It also stops waiting
/// the moment `stop` says the answer is no longer wanted (spec 082 RJ4: the
/// core has answered the page from the tracker), looked at a few times a
/// second. The call itself runs on to its end inside the pool (its verdicts
/// still count); only this caller stops waiting, and a late answer goes
/// nowhere.
pub fn bundler_call_until(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
    stop: &dyn Fn() -> bool,
) -> Result<Value, PoolError> {
    route_within(chain_id, RpcKind::Bundler, method, params, budget, stop)
}

/// [`call`], waiting at most `budget` — for a best-effort read that must not
/// hold its caller for a whole sweep (the submit's head read, spec 082).
pub fn call_within(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    route_within(chain_id, RpcKind::Rpc, method, params, budget, &|| false)
}

/// [`bundler_call`], waiting at most `budget` for the answer (spec 079) —
/// the dApp's landing wait (083) asks the relay this way.
pub fn bundler_call_within(
    chain_id: u32,
    method: &str,
    params: Value,
    budget: Duration,
) -> Result<Value, PoolError> {
    route_within(chain_id, RpcKind::Bundler, method, params, budget, &|| {
        false
    })
}

fn route_within(
    chain_id: u32,
    kind: RpcKind,
    method: &str,
    params: Value,
    budget: Duration,
    stop: &dyn Fn() -> bool,
) -> Result<Value, PoolError> {
    /// How often a waiting caller looks at `stop`.
    const GLANCE: Duration = Duration::from_millis(200);
    if faulted(chain_id) {
        return Err(PoolError::Unavailable);
    }
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
    let deadline = Instant::now() + budget;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stop() {
            return Err(PoolError::Failed {
                rate_limited: false,
            });
        }
        match answer.recv_timeout(left.min(GLANCE)) {
            Ok(routed) => return routed.answer,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(PoolError::Unavailable);
            }
        }
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

/// The RPC this wallet uses for a chain, as the core names it to the relay
/// (`getChainRpcUrl`, the core's `answer_best_rpc_url`): the same eligible set
/// the JSON-RPC bundler leg reads, so the REST and JSON-RPC doors to the relay
/// name the same endpoint. `None` when the core has none to vouch for — the
/// caller then sends no header. **Blocks.** (Spec 098 §5.)
pub fn best_rpc_url(chain_id: u32) -> Option<String> {
    query(chain_id, |chain_id, reply| Request::BestRpcUrl {
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

/// `VELA_FAULT_POOL=1,100|all` — a developer seam (PR 2 note 11; the web's
/// `vela.faultPool(chain)`): every routed call to a named chain answers
/// [`PoolError::Unavailable`], as if the pool thread were gone, so the
/// app's own fault can be seen where it is said — the home's line, the fee
/// row, Continue's alert — without breaking anything real. Read through
/// `dev_env`, so a release build does not carry it.
fn faulted(chain_id: u32) -> bool {
    #[cfg(test)]
    if let Some(chains) = TEST_FAULT.with(|fault| fault.borrow().clone()) {
        return chains.contains(&chain_id);
    }
    crate::dev_env::var!("VELA_FAULT_POOL").is_some_and(|list| {
        list.trim().eq_ignore_ascii_case("all")
            || list
                .split(',')
                .any(|id| id.trim().parse::<u32>() == Ok(chain_id))
    })
}

#[cfg(test)]
thread_local! {
    static TEST_FAULT: std::cell::RefCell<Option<Vec<u32>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `work` with the pool faulted for `chains` on this thread only
/// ([`faulted`]), so a test proves the classification without touching the
/// process's environment or the tests beside it.
#[cfg(test)]
pub fn with_fault<T>(chains: Option<&[u32]>, work: impl FnOnce() -> T) -> T {
    TEST_FAULT.with(|fault| *fault.borrow_mut() = chains.map(<[u32]>::to_vec));
    let out = work();
    TEST_FAULT.with(|fault| *fault.borrow_mut() = None);
    out
}

fn route(chain_id: u32, kind: RpcKind, method: &str, params: Value) -> Routed {
    if faulted(chain_id) {
        return Routed::unavailable();
    }
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

/// How many hedges may be on the wire at once, process-wide (083 H6).
///
/// Their own budget, deliberately NOT a share of [`MAX_WORKERS`]: at that
/// ceiling the core's own post runs inline on the pool thread, which is every
/// caller in the process waiting — the queue of one this file exists to
/// avoid. A hedge is an optimisation and must never be the reason a real
/// post loses its worker; past this budget a read simply is not hedged, and
/// the core's sweep answers as it did before hedging.
const MAX_HEDGES: usize = MAX_WORKERS / 4;

static HEDGES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Workers the core's own operations keep to themselves (083 H6): a hedge
/// goes out only while fewer than `MAX_WORKERS - WORKER_RESERVE` of them are
/// busy. A caller a hedge answered sooner asks again sooner, and until the
/// core has seen a silent node's first timeout every new read still goes
/// there first and holds a worker for the whole read timeout. Near the
/// ceiling hedging stops, callers wait out the core's own sweep again, and
/// that back-pressure is what keeps the core's posts off the pool thread.
const WORKER_RESERVE: usize = MAX_WORKERS / 4;

/// A blocking operation handed to a worker thread, and what it reports back.
type Work = Box<dyn FnOnce() -> Message + Send>;

/// What the shell is holding for one in-flight call.
struct InFlight {
    params: Value,
    /// Taken by whoever answers the caller first: the core's verdict, or a
    /// hedge whose answer the core's [`early_verdict`] lets through (083 H6).
    reply: Option<Sender<Routed>>,
    /// The last body received, per URL. `Conclude { Respond { url } }` names
    /// which one the core accepted — the core never sees a body itself.
    bodies: HashMap<String, Value>,
    /// Only a read the core calls hedged ([`is_hedged_read`]) has one.
    hedge: Option<Hedge>,
    /// For the log lines and the network-health count only.
    chain_id: u32,
    kind: RpcKind,
    method: String,
    started: Instant,
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
/// endpoint EARLY, and only the endpoint the core will ask next: the first of
/// the current pass's remaining queue (`RpcPoolView::pending_urls`) not
/// already hedged and not banned since the pass began. An endpoint the core
/// has set aside — cooling after a timeout, backing off after a 429 — sits at
/// the back of that queue, so it is hedged only when the core would reach it
/// anyway. When the core does reach a hedged endpoint the outcome is already
/// in hand (or on the wire) and is used instead of a second request, so a
/// pass asks each endpoint once, hedged or not, exactly as the core would.
///
/// The caller may take a hedge's answer before the core reaches it only when
/// the core's [`early_verdict`] says it is an answer every node gives alike;
/// anything else waits for the core's order. The loser is abandoned: once the
/// core concludes, a hedge that comes back later is dropped.
struct Hedge {
    method: String,
    /// The post the core is waiting on, and when it went out.
    current: Option<(String, Instant)>,
    /// Hedges on the wire: when each went out, and the core's operation id
    /// once the core has reached that endpoint and waits on it too.
    flying: HashMap<String, (Instant, Option<u64>)>,
    /// Hedges that came back before the core asked for their endpoint.
    landed: HashMap<String, RpcShellResult>,
    /// Nothing is left in the core's queue to hedge its current post to:
    /// nothing more is sent until the core moves on to its next post.
    stalled: bool,
    /// No hedge worker was free when this read was due: it tries again at
    /// this instant rather than giving up on the post.
    retry: Option<Instant>,
}

/// What the core met when it reached an endpoint of a hedged read.
enum Reached {
    /// A hedge already has the outcome: it is the core's answer.
    Landed(RpcShellResult),
    /// A hedge is on the wire there: the core waits on it, no second post.
    Flying,
    /// Nobody has asked it yet: the core's own post goes out.
    Fresh,
}

impl Hedge {
    /// `Some` for a read that may be hedged; a write, and anything for the
    /// bundler, never is.
    fn for_call(kind: RpcKind, method: &str) -> Option<Self> {
        is_hedged_read(kind, method).then(|| Self {
            method: method.to_owned(),
            current: None,
            flying: HashMap::new(),
            landed: HashMap::new(),
            stalled: false,
            retry: None,
        })
    }

    /// The core's post has been out [`HEDGE_AFTER`] and no earlier hedge is
    /// still unanswered: one extra request on the wire per read, never a
    /// fan-out.
    fn due(&self) -> Option<Instant> {
        if self.stalled || self.flying.values().any(|(_, core)| core.is_none()) {
            return None;
        }
        let (_, sent) = self.current.as_ref()?;
        let due = *sent + HEDGE_AFTER;
        Some(self.retry.map_or(due, |retry| retry.max(due)))
    }

    /// The endpoint the core will ask next: the first of its remaining queue
    /// (`pending`, best first) not already hedged this pass and not banned
    /// since the pass was drawn up. No order of this file's own.
    fn target(&self, pending: &[String], banned: &[RpcBanEntry], now_ms: f64) -> Option<String> {
        pending
            .iter()
            .find(|url| {
                !self.flying.contains_key(*url)
                    && !self.landed.contains_key(*url)
                    && !banned
                        .iter()
                        .any(|ban| ban.url == **url && is_ban_active(ban, now_ms))
            })
            .cloned()
    }

    /// The core has reached `url` with its operation `id`.
    fn reached(&mut self, url: &str, id: u64) -> Reached {
        // The core moved on: its next post may be hedged afresh.
        self.stalled = false;
        self.retry = None;
        if let Some(result) = self.landed.remove(url) {
            return Reached::Landed(result);
        }
        if let Some((sent, core)) = self.flying.get_mut(url) {
            *core = Some(id);
            self.current = Some((url.to_owned(), *sent));
            return Reached::Flying;
        }
        self.current = Some((url.to_owned(), Instant::now()));
        Reached::Fresh
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
        hedge_due_reads(|| host.view(), &mut inflight, &mut |work| {
            spawn_hedge(workers, work)
        });
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
                        chain_id,
                        kind,
                        method: method.clone(),
                        started: Instant::now(),
                        hedge: Hedge::for_call(kind, &method),
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
            Request::BestRpcUrl { chain_id, reply } => {
                next_id += 1;
                let call_id = format!("r{next_id}");
                queries.insert(call_id.clone(), reply);
                let pending = host.dispatch(Event::BestRpcUrlRequested {
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
            match hedge.reached(url, next.id) {
                Reached::Landed(result) => {
                    queue.extend(host.resolve(next.id, result));
                    continue;
                }
                Reached::Flying => continue,
                Reached::Fresh => {}
            }
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

/// Send every read whose current endpoint has been silent for [`HEDGE_AFTER`]
/// to the endpoint the core will ask next (083 H6).
///
/// `view` is the core's (its queues and its ban truth), read once per round
/// and only when some read is due. `spawn` hands a hedge to a worker
/// ([`spawn_hedge`]); `false` means none was free, and the read tries again
/// after [`HEDGE_RETRY`].
fn hedge_due_reads(
    mut view: impl FnMut() -> RpcPoolView,
    inflight: &mut HashMap<String, InFlight>,
    spawn: &mut impl FnMut(Work) -> bool,
) {
    let now = Instant::now();
    let mut core: Option<RpcPoolView> = None;
    for (call_id, call) in inflight.iter_mut() {
        if !call.hedge_due().is_some_and(|due| due <= now) {
            continue;
        }
        let core = core.get_or_insert_with(&mut view);
        let Some(hedge) = call.hedge.as_mut() else {
            continue;
        };
        let pending = core
            .pending_urls
            .get(call_id)
            .map_or(&[][..], Vec::as_slice);
        let Some(url) = hedge.target(pending, &core.banned, now_ms()) else {
            hedge.stalled = true;
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
        // No hedge worker free: not sent now, tried again shortly; the
        // core's sweep answers meanwhile, as it did before hedging.
        if !spawn(work) {
            hedge.retry = Some(now + HEDGE_RETRY);
            continue;
        }
        hedge.retry = None;
        hedge.flying.insert(url, (now, None));
    }
}

/// A hedge came back (083 H6). The caller has it now only if the core says
/// it is an answer every node gives alike ([`early_verdict`]): a result or a
/// revert. A range cap, an unsupported method — a fact about THAT node — is
/// the core's to conclude with when it reaches that endpoint in its own
/// order, after the endpoints before it have failed; handed over here, it
/// would beat the preferred node's real answer whenever that node was merely
/// slower than the hedge delay. The core has every outcome when it reaches
/// that endpoint — at once, as the returned operation, when it already waits
/// there.
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
    if let Some(verdict) = early_verdict(RpcKind::Rpc, &hedge.method, url, outcome)
        && let Some(reply) = call.reply.take()
    {
        let _ = reply.send(routed_of(&verdict, &call.bodies));
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
    let work: Work = match operation {
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
    spawn_worker(workers, work, &WORKERS, MAX_WORKERS)
}

/// Hand a hedge to a worker from the hedges' own budget (083 H6) — and only
/// while the core's own operations have [`WORKER_RESERVE`] workers to spare,
/// so a hedge is never the reason a core post runs on the pool thread.
fn spawn_hedge(workers: &Sender<Message>, work: Work) -> bool {
    use std::sync::atomic::Ordering;

    WORKERS.load(Ordering::Relaxed) < MAX_WORKERS - WORKER_RESERVE
        && spawn_worker(workers, work, &HEDGES, MAX_HEDGES)
}

/// Run `work` on a worker thread that reports back through the channel,
/// counted against `budget` — [`WORKERS`] for the core's own operations,
/// [`HEDGES`] for hedges, so neither can take the other's threads. `false`
/// at `ceiling`, or when the OS refuses a thread. Only the pool thread
/// spawns, so the check and the increment cannot race each other.
fn spawn_worker(
    workers: &Sender<Message>,
    work: Work,
    budget: &'static std::sync::atomic::AtomicUsize,
    ceiling: usize,
) -> bool {
    use std::sync::atomic::Ordering;

    if budget.load(Ordering::Relaxed) >= ceiling {
        return false;
    }
    budget.fetch_add(1, Ordering::Relaxed);
    let workers = workers.clone();
    let spawned = std::thread::Builder::new()
        .name("vela-rpc-work".to_owned())
        .spawn(move || {
            let message = work();
            budget.fetch_sub(1, Ordering::Relaxed);
            // The pool thread is gone only if it panicked; there is nobody
            // left to tell, and the caller's own reply channel closing is
            // what reports it.
            let _ = workers.send(message);
        });
    if spawned.is_err() {
        budget.fetch_sub(1, Ordering::Relaxed);
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
            // A caller a hedge already answered has no reply left (083 H6);
            // the core's verdict still closed the call, and its books.
            if let Some(call) = inflight.remove(call_id) {
                note_conclusion(&call, verdict);
                if let Some(reply) = call.reply {
                    // A caller that gave up is not an error: the receiver is simply
                    // gone, and the pool has nothing to be sad about.
                    let _ = reply.send(routed_of(verdict, &call.bodies));
                }
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

/// "Went offline" edges since launch — for the tests that prove one
/// failing chain never raises one.
static WENT_OFFLINE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Feed one conclusion into the count: which chain it read (spec 082 RJ14,
/// G53 — the count is of sources, not calls, so one chain whose nodes all
/// fail while the others answer is that chain's notice, never "offline"),
/// and when. `Some(edge)` when it crossed one.
fn feed_health(reached: bool, chain_id: u32, now_ms: f64) -> Option<NetEdge> {
    let Ok(mut health) = HEALTH.lock() else {
        return None;
    };
    let (next, edge) = step_health(&health, reached, chain_id, now_ms);
    *health = next;
    edge
}

/// One pooled read into the core's count, sourced by its chain.
fn step_health(
    health: &NetHealth,
    reached: bool,
    chain_id: u32,
    now_ms: f64,
) -> (NetHealth, Option<NetEdge>) {
    net_health_step(health.clone(), reached, Some(chain_id), now_ms)
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
    match feed_health(reached, call.chain_id, now_ms()) {
        Some(NetEdge::WentOffline) => {
            WENT_OFFLINE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            vlog!("net", "offline (chain reads stopped answering)");
        }
        Some(NetEdge::CameBack) => {
            vlog!("net", "came back");
            CAME_BACK.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // RE3: a balance read on the way back, not at the next 10 min
            // pass — one that joins a read already out (G53: every "came
            // back" used to start a second round beside the first).
            crate::executor::balance_dashboard::network_back();
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
        // Spec 098 §5: a bundler call names the chain's RPC to the relay, under
        // the name the relay reads. The core sets `x_rpc_url` on bundler calls
        // only, so an RPC provider is never sent one.
        let request = agent.post(url).header("content-type", "application/json");
        match x_rpc_url {
            Some(rpc) => request
                .header(super::relay::RELAY_RPC_URL_HEADER, rpc)
                .send_json(&payload),
            None => request.send_json(&payload),
        }
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
    // A test's loopback pool, in an order and tiers the stored settings
    // cannot express (they hold two endpoints per chain at most).
    #[cfg(test)]
    if let Some(rpc) = tests::seeded_pool(chain_id) {
        return (rpc, Vec::new());
    }

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

    // 4. the curated public fallbacks — the CORE's list
    // (`network_admin::PUBLIC_RPCS`), one table for every shell. The desktop
    // kept its own copy, and that copy still named `1rpc.io`, which timed
    // out on every call (issue 483's research): a fallback that never
    // answers is a slower failure, not a second chance.
    for url in vela_core::app::network_admin::public_rpc_urls(chain_id) {
        add(url, RpcSource::Public, &mut rpc);
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

    /// Spec 082 RJ14 (G53): the count is of sources, not calls. Ten Gnosis
    /// sweeps that give up — while the other chains answer, and even with
    /// nothing else asked at all — are Gnosis's notice, never "offline". The
    /// device log had ten `net:` lines in two minutes, each "came back"
    /// 0.2–0.9 s after "offline", with chains 1, 100 and 480 failing and the
    /// rest answering; each "came back" forced another balance round.
    #[test]
    fn ten_gnosis_misses_never_go_offline() {
        fn feed(
            health: &mut NetHealth,
            edges: &mut Vec<NetEdge>,
            reached: bool,
            chain: u32,
            at: f64,
        ) {
            let (next, edge) = step_health(health, reached, chain, at);
            *health = next;
            edges.extend(edge);
        }
        let mut edges = Vec::new();
        let mut now = 1_757_000_000_000.0;

        let mut health = NetHealth::default();
        for miss in 0..10 {
            feed(&mut health, &mut edges, false, 100, now);
            if miss % 3 == 2 {
                feed(&mut health, &mut edges, true, 8453, now + 100.0);
            }
            now += 4_000.0;
        }
        assert!(
            edges.is_empty(),
            "one chain failing is not offline: {edges:?}"
        );

        // Gnosis alone, nothing else asked, far past the quiet window.
        let mut health = NetHealth::default();
        for _ in 0..10 {
            feed(&mut health, &mut edges, false, 100, now);
            now += 4_000.0;
        }
        assert!(edges.is_empty(), "still one source: {edges:?}");
        assert!(health.online);

        // The network really gone: two chains' sweeps give up with nothing
        // reached for ten seconds — offline once; the first answer after it
        // is "came back", once.
        let mut health = NetHealth::default();
        feed(&mut health, &mut edges, true, 8453, now);
        for chain in [100, 1, 100, 1] {
            now += 4_000.0;
            feed(&mut health, &mut edges, false, chain, now);
        }
        assert_eq!(edges, vec![NetEdge::WentOffline]);
        feed(&mut health, &mut edges, true, 8453, now + 500.0);
        assert_eq!(edges, vec![NetEdge::WentOffline, NetEdge::CameBack]);
    }

    /// Spec 082 T224, G53's relaunch, through the real pool: one chain whose
    /// endpoint takes no connection and 23 that answer, read twice over as
    /// two balance rounds would (the second overlapping the first). Every
    /// answering chain answers, promptly; the pool fails that one chain and
    /// no other; and no "offline" edge is raised for it.
    #[test]
    fn one_dead_chain_among_answering_ones_fails_alone() {
        const DEAD: u32 = 424_400;
        const FIRST: u32 = 424_401;
        const ANSWERING: u32 = 23;

        storage::tests::with_temp_state("pool-one-dead-chain", || {
            // A port nothing listens on: the connect is refused. It is HELD
            // while the answering chains take their ports, and let go only
            // then — freed first, the next `bind(0)` could be handed the same
            // port, and the "dead" chain answered as one of the live ones
            // (CI, PR #426: one such run poisoned the storage lock under 105
            // other tests).
            let dead = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap_or_else(|error| unreachable!("no loopback port: {error}"));
            let dead_port = dead
                .local_addr()
                .map(|addr| addr.port())
                .unwrap_or_else(|error| unreachable!("no port: {error}"));
            let mut networks =
                vec![json!({ "chainId": DEAD, "rpcURL": format!("http://127.0.0.1:{dead_port}") })];
            for chain in FIRST..FIRST + ANSWERING {
                let port = fake_rpc(chain, Duration::ZERO);
                networks.push(
                    json!({ "chainId": chain, "rpcURL": format!("http://127.0.0.1:{port}") }),
                );
            }
            drop(dead);
            if storage::write_value(storage::KEY_CUSTOM_NETWORKS, Value::Array(networks)).is_err() {
                unreachable!("could not seed the networks");
            }
            let offline_before = WENT_OFFLINE.load(std::sync::atomic::Ordering::SeqCst);

            let round = || {
                let mut calls = Vec::new();
                for chain in std::iter::once(DEAD).chain(FIRST..FIRST + ANSWERING) {
                    for method in ["eth_getBalance", "eth_blockNumber", "eth_call"] {
                        calls.push(std::thread::spawn(move || {
                            let began = Instant::now();
                            let answer = call(chain, method, json!([]));
                            (chain, answer.is_ok(), began.elapsed())
                        }));
                    }
                }
                calls
            };
            let mut calls = round();
            std::thread::sleep(Duration::from_millis(200));
            calls.extend(round());
            for handle in calls {
                let (chain, answered, took) = handle
                    .join()
                    .unwrap_or_else(|_| unreachable!("a caller panicked"));
                if chain == DEAD {
                    assert!(!answered, "the dead chain answered");
                } else {
                    assert!(answered, "chain {chain} did not answer");
                    assert!(
                        took < Duration::from_secs(5),
                        "chain {chain} waited {took:?} behind the dead one"
                    );
                }
            }

            let failed = failed_chains();
            assert!(
                failed.contains(&DEAD),
                "the dead chain is failed: {failed:?}"
            );
            assert!(
                !failed
                    .iter()
                    .any(|chain| (FIRST..FIRST + ANSWERING).contains(chain)),
                "an answering chain is failed: {failed:?}"
            );
            assert_eq!(
                WENT_OFFLINE.load(std::sync::atomic::Ordering::SeqCst),
                offline_before,
                "one dead chain raised \"offline\""
            );
        });
    }

    /// PR 3 item 11: the public tier is the CORE's curated list
    /// (`network_admin::public_rpc_urls`), in its order, for every built-in
    /// network — one table for every shell. The desktop's own copy still
    /// named `1rpc.io`, which had stopped answering; no endpoint of that
    /// host is seeded on any chain any more.
    #[test]
    fn the_public_tier_is_the_cores_curated_list() {
        storage::tests::with_temp_state("pool-public-tier", || {
            use vela_core::app::network_admin::public_rpc_urls;
            let mut with_a_public_tier = 0;
            for chain in BUILTIN_CHAINS {
                let (rpc, _) = collect_endpoints(chain.chain_id);
                let public: Vec<&str> = rpc
                    .iter()
                    .filter(|seed| seed.source == RpcSource::Public)
                    .map(|seed| seed.url.as_str())
                    .collect();
                // The core's list, less any URL that is also the chain's
                // default (offered once, at its own tier).
                let expected: Vec<String> = public_rpc_urls(chain.chain_id)
                    .into_iter()
                    .filter(|url| url != chain.rpc_url)
                    .collect();
                assert_eq!(public, expected, "chain {}", chain.chain_id);
                assert!(
                    rpc.iter().all(|seed| !seed.url.contains("1rpc.io")),
                    "chain {}: a dead fallback is still offered",
                    chain.chain_id
                );
                with_a_public_tier += usize::from(!expected.is_empty());
            }
            assert!(
                with_a_public_tier >= 9,
                "the curated list covers the majors"
            );
            // Polygon, where the dead fallback was measured: a second
            // endpoint that answers.
            let (polygon, _) = collect_endpoints(137);
            assert!(
                polygon
                    .iter()
                    .any(|seed| seed.url == "https://polygon.gateway.tenderly.co"),
                "{:?}",
                polygon.iter().map(|seed| &seed.url).collect::<Vec<_>>()
            );
        });
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

    /// The header block of the one request a loopback server receives, lower-cased.
    fn headers_of_one_post(x_rpc_url: Option<&str>) -> String {
        use std::io::{BufRead as _, Read as _, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|error| unreachable!("no loopback port: {error}"));
        let port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_else(|error| unreachable!("no port: {error}"));
        let (seen_tx, seen) = channel();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = std::io::BufReader::new(
                stream
                    .try_clone()
                    .unwrap_or_else(|error| unreachable!("clone: {error}")),
            );
            let (mut head, mut line, mut length) = (String::new(), String::new(), 0usize);
            while reader.read_line(&mut line).unwrap_or(0) > 0 && line != "\r\n" {
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                head.push_str(&line.to_lowercase());
                line.clear();
            }
            let mut body = vec![0u8; length];
            let _ = reader.read_exact(&mut body);
            let payload = r#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = seen_tx.send(head);
        });
        let url = format!("http://127.0.0.1:{port}/");
        let _ = post(&url, "eth_sendUserOperation", &json!([]), x_rpc_url, 5_000);
        seen.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| unreachable!("the server never saw the request"))
    }

    /// Spec 098 §5: a bundler call names the chain's RPC to the relay, under
    /// the name the relay reads — key and all.
    #[test]
    fn a_bundler_post_names_the_chains_rpc_to_the_relay() {
        let head = headers_of_one_post(Some("https://rpc.one/v2/KEY"));
        assert!(
            head.contains("x-vela-rpc-url: https://rpc.one/v2/key"),
            "the relay was not told the RPC: {head}"
        );
        assert!(
            !head.contains("x-rpc-url:"),
            "the pre-081 name the relay never read"
        );
    }

    /// …and a call the core did not mark as a bundler call names none.
    #[test]
    fn a_plain_rpc_post_names_no_rpc() {
        let head = headers_of_one_post(None);
        assert!(
            !head.contains("x-vela-rpc-url"),
            "an RPC provider was sent an RPC: {head}"
        );
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
        scripted_node_seq(vec![(delay, response)])
    }

    /// [`scripted_node`], answering its n-th request by `script[n]` — and
    /// every request past the script by its last entry.
    fn scripted_node_seq(
        script: Vec<(Duration, Option<String>)>,
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
        let script = std::sync::Arc::new(script);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (seen, script) = (std::sync::Arc::clone(&seen), std::sync::Arc::clone(&script));
                std::thread::spawn(move || {
                    let _ = read_request(&stream);
                    let index = seen.lock().map_or(0, |mut seen| {
                        seen.push(Instant::now());
                        seen.len().saturating_sub(1)
                    });
                    let (delay, response) = script
                        .get(index)
                        .or_else(|| script.last())
                        .cloned()
                        .unwrap_or((Duration::ZERO, None));
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

    /// A JSON-RPC answer whose `result` is `result`.
    fn result_body(result: &str) -> String {
        format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{result}"}}"#)
    }

    /// The `result` member a caller received, if it received one.
    fn result_of(answer: &Result<Value, PoolError>) -> Option<Value> {
        answer
            .as_ref()
            .ok()
            .and_then(|body| body.get("result").cloned())
    }

    fn arrivals(log: &Mutex<Vec<Instant>>) -> Vec<Instant> {
        log.lock().map(|seen| seen.clone()).unwrap_or_default()
    }

    /// Chains whose pool a test hands in whole (see `collect_endpoints`).
    static SEEDED: Mutex<Vec<(u32, Vec<RpcEndpointSeed>)>> = Mutex::new(Vec::new());

    pub(super) fn seeded_pool(chain_id: u32) -> Option<Vec<RpcEndpointSeed>> {
        let seeded = SEEDED.lock().ok()?;
        seeded
            .iter()
            .find(|(id, _)| *id == chain_id)
            .map(|(_, rpc)| rpc.clone())
    }

    /// One chain whose pool is exactly `nodes`, with these tiers.
    fn seed_pool(chain_id: u32, nodes: &[(&str, RpcSource)]) {
        let Ok(mut seeded) = SEEDED.lock() else {
            unreachable!("the seed lock is poisoned");
        };
        seeded.retain(|(id, _)| *id != chain_id);
        seeded.push((
            chain_id,
            nodes
                .iter()
                .map(|(url, source)| RpcEndpointSeed {
                    url: (*url).to_owned(),
                    source: *source,
                })
                .collect(),
        ));
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

    /// Wait, at most `within`, for the pool to close every call; the count
    /// still open.
    fn settle(within: Duration) -> usize {
        let began = Instant::now();
        loop {
            let open = open_calls();
            if open == 0 || began.elapsed() >= within {
                return open;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
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
            assert!(Hedge::for_call(RpcKind::Rpc, "eth_sendRawTransaction").is_none());
            assert!(Hedge::for_call(RpcKind::Bundler, "eth_getUserOperationReceipt").is_none());
            assert!(Hedge::for_call(RpcKind::Rpc, "eth_call").is_some());
        });
    }

    /// 083 H6: when every node fails, the caller hears the pool's failure,
    /// classified from the nodes in the core's order as it always was — never
    /// the hedge's own failure in its place. The first node is rate-limited
    /// (429, slowly) and the second refuses at once (400 — it answered): the
    /// verdict is the first node's "rate-limited", the transient case the
    /// balance screen keeps quiet about (invariant ④), not the second node's
    /// plain failure. (A 5xx there would be a node not reached, and since spec
    /// 092 that makes the chain failed rather than busy.)
    #[test]
    fn when_both_nodes_fail_the_first_nodes_failure_is_reported() {
        const CHAIN: u32 = 424_246;
        const SLOW: Duration = Duration::from_millis(2_000);

        storage::tests::with_temp_state("pool-hedge-fail", || {
            let (first, _) = scripted_node(SLOW, Some(http("429 Too Many Requests", "")));
            let (second, second_log) =
                scripted_node(Duration::ZERO, Some(http("400 Bad Request", "")));
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

    /// One round of the real `hedge_due_reads` against `view`, with a hedge
    /// worker free or none: how many times it read the core's view, and how
    /// many hedges it sent.
    fn hedge_round(
        view: &RpcPoolView,
        inflight: &mut HashMap<String, InFlight>,
        free: bool,
    ) -> (usize, usize) {
        let (mut views, mut sent) = (0, 0);
        hedge_due_reads(
            || {
                views += 1;
                view.clone()
            },
            inflight,
            &mut |_work| {
                sent += usize::from(free);
                free
            },
        );
        (views, sent)
    }

    fn the_hedge(inflight: &mut HashMap<String, InFlight>) -> &mut Hedge {
        inflight
            .get_mut("r1")
            .and_then(|call| call.hedge.as_mut())
            .unwrap_or_else(|| unreachable!("r1 is a hedged read"))
    }

    /// The core posts to `url`, and that post has been silent two seconds:
    /// a hedge is due.
    fn silent_for_two_seconds(hedge: &mut Hedge, url: &str, id: u64) {
        assert!(matches!(hedge.reached(url, id), Reached::Fresh));
        let then = Instant::now()
            .checked_sub(Duration::from_secs(2))
            .unwrap_or_else(|| unreachable!("the clock is younger than two seconds"));
        hedge.current = Some((url.to_owned(), then));
    }

    /// A hedge's outcome, landed before the core reached its endpoint.
    fn land(hedge: &mut Hedge, url: &str) {
        assert!(hedge.flying.remove(url).is_some(), "{url} was not hedged");
        hedge.landed.insert(
            url.to_owned(),
            RpcShellResult::PostOutcome {
                call_id: "r1".to_owned(),
                url: url.to_owned(),
                outcome: RpcTransportOutcome::Timeout,
                latency_ms: 8_000.0,
                now_ms: now_ms(),
            },
        );
    }

    fn flying(inflight: &mut HashMap<String, InFlight>) -> Vec<String> {
        the_hedge(inflight).flying.keys().cloned().collect()
    }

    /// 083 H6, the real round (`hedge_due_reads`, review finding 5): a due
    /// read is hedged to the endpoint the CORE will ask next — the first of
    /// its remaining queue, whatever the tiers say — with one hedge
    /// unanswered at a time, never to an endpoint banned since the pass
    /// began, and not at all once the queue is spent; the core's next post
    /// lifts that. A read that found no hedge worker free tries again after
    /// `HEDGE_RETRY` instead of giving up on the post (review finding 3). The
    /// view is read only when a read is due.
    #[test]
    fn a_due_read_is_hedged_down_the_cores_own_queue() {
        let now = now_ms();
        let mut view = RpcPoolView {
            failed_chains: Vec::new(),
            rate_limited_chains: Vec::new(),
            banned: vec![RpcBanEntry {
                url: "https://c".to_owned(),
                banned_at_ms: now - 1_000.0,
                permanent: false,
            }],
            // The core's own order: `https://tier-one` is the pool's first
            // tier, cooling after a timeout, so the core put it last.
            pending_urls: [(
                "r1".to_owned(),
                ["https://b", "https://c", "https://d", "https://tier-one"]
                    .map(str::to_owned)
                    .to_vec(),
            )]
            .into_iter()
            .collect(),
            unreached_chains: Vec::new(),
        };
        let Some(hedge) = Hedge::for_call(RpcKind::Rpc, "eth_call") else {
            unreachable!("eth_call is a read");
        };
        assert_eq!(hedge.due(), None, "nothing to hedge before the core posts");
        let (reply, _answer) = channel();
        let mut inflight = HashMap::from([(
            "r1".to_owned(),
            InFlight {
                params: json!([]),
                reply: Some(reply),
                bodies: HashMap::new(),
                hedge: Some(hedge),
                chain_id: 100,
                kind: RpcKind::Rpc,
                method: "eth_call".to_owned(),
                started: Instant::now(),
            },
        )]);

        // Not yet due: the core's post has only just gone out.
        assert!(matches!(
            the_hedge(&mut inflight).reached("https://a", 1),
            Reached::Fresh
        ));
        assert_eq!(hedge_round(&view, &mut inflight, true), (0, 0));

        silent_for_two_seconds(the_hedge(&mut inflight), "https://a", 1);
        assert_eq!(hedge_round(&view, &mut inflight, true), (1, 1));
        assert_eq!(flying(&mut inflight), ["https://b"]);
        // One unanswered hedge at a time.
        assert_eq!(hedge_round(&view, &mut inflight, true), (0, 0));

        // B lands failing; C was banned since the pass began, so D is next.
        land(the_hedge(&mut inflight), "https://b");
        assert_eq!(hedge_round(&view, &mut inflight, true), (1, 1));
        assert_eq!(flying(&mut inflight), ["https://d"]);

        // The core's last, cooling endpoint is hedged only after every
        // endpoint the core puts before it.
        land(the_hedge(&mut inflight), "https://d");
        assert_eq!(hedge_round(&view, &mut inflight, true), (1, 1));
        assert_eq!(flying(&mut inflight), ["https://tier-one"]);
        land(the_hedge(&mut inflight), "https://tier-one");

        // The queue is spent: nothing is sent, and nothing more is due.
        assert_eq!(hedge_round(&view, &mut inflight, true), (1, 0));
        assert_eq!(the_hedge(&mut inflight).due(), None);
        assert_eq!(hedge_round(&view, &mut inflight, true), (0, 0));

        // The core reaches a landed hedge: its outcome, no second post.
        assert!(matches!(
            the_hedge(&mut inflight).reached("https://b", 2),
            Reached::Landed(_)
        ));

        // A new pass. No hedge worker free: the read waits `HEDGE_RETRY`,
        // neither spinning nor giving up on this post.
        view.pending_urls
            .insert("r1".to_owned(), vec!["https://e".to_owned()]);
        silent_for_two_seconds(the_hedge(&mut inflight), "https://a", 3);
        assert_eq!(hedge_round(&view, &mut inflight, false), (1, 0));
        assert!(flying(&mut inflight).is_empty());
        let retry = the_hedge(&mut inflight).due();
        assert!(
            retry.is_some_and(|at| at > Instant::now() && at <= Instant::now() + HEDGE_RETRY),
            "{retry:?}"
        );
        assert_eq!(
            hedge_round(&view, &mut inflight, true),
            (0, 0),
            "retried at once"
        );
        std::thread::sleep(HEDGE_RETRY);
        assert_eq!(hedge_round(&view, &mut inflight, true), (1, 1));
        assert_eq!(flying(&mut inflight), ["https://e"]);

        // A caller with its answer is hedged no further.
        land(the_hedge(&mut inflight), "https://e");
        view.pending_urls
            .insert("r1".to_owned(), vec!["https://g".to_owned()]);
        if let Some(call) = inflight.get_mut("r1") {
            call.reply = None;
        }
        assert_eq!(hedge_round(&view, &mut inflight, true), (0, 0));
    }

    /// 083 H6, review finding 2: the hedge goes where the CORE goes next,
    /// not down the tiers. A — the user's override, the first tier —
    /// swallows a read and fails, so the core cools it for 30 s and puts it
    /// LAST. On the next read B, first now, is slow: the hedge must go to C,
    /// which answers, and A must not be asked again. A tier walk sent that
    /// hedge to the cooling A and waited on it.
    #[test]
    fn a_hedge_goes_where_the_core_goes_next_not_to_a_cooling_node() {
        const CHAIN: u32 = 424_247;

        storage::tests::with_temp_state("pool-hedge-order", || {
            let (a, a_log) = scripted_node(Duration::from_secs(2), None);
            let (b, b_log) = scripted_node_seq(vec![
                (Duration::ZERO, Some(http("200 OK", &result_body("0xb")))),
                (
                    Duration::from_millis(2_500),
                    Some(http("200 OK", &result_body("0xb"))),
                ),
            ]);
            let (c, c_log) =
                scripted_node(Duration::ZERO, Some(http("200 OK", &result_body("0xc"))));
            seed_pool(
                CHAIN,
                &[
                    (&a, RpcSource::User),
                    (&b, RpcSource::Default),
                    (&c, RpcSource::Public),
                ],
            );

            // Read 1: A is silent, the hedge to B answers, then A fails.
            let first = call(CHAIN, "eth_call", json!([]));
            assert_eq!(result_of(&first), Some(json!("0xb")), "{first:?}");
            assert_eq!(settle(Duration::from_secs(3)), 0, "read 1 was left open");
            assert_eq!(arrivals(&a_log).len(), 1);

            // Read 2: the core asks B, slow now; next it would ask C, and
            // the cooling A only after that.
            let began = Instant::now();
            let second = call(CHAIN, "eth_call", json!([]));
            let waited = began.elapsed();
            assert_eq!(
                result_of(&second),
                Some(json!("0xc")),
                "the hedge did not go to C: {second:?}"
            );
            assert!(
                waited < HEDGE_AFTER + Duration::from_millis(900),
                "{waited:?}"
            );
            assert_eq!(settle(Duration::from_secs(3)), 0, "read 2 was left open");
            assert_eq!(arrivals(&a_log).len(), 1, "the cooling node was hedged");
            assert_eq!(arrivals(&b_log).len(), 2);
            assert_eq!(arrivals(&c_log).len(), 1);
        });
    }

    /// 083 H6, review finding 1: a hedged node's cap or gap never beats the
    /// preferred node's real answer. The first node is merely slow (2.5 s);
    /// the hedged second node answers at once with a fact about ITSELF — a
    /// getLogs range cap, a -32601 "method does not exist" — and the caller
    /// still gets the first node's answer. A revert, which every node gives
    /// alike, is still the caller's the moment the hedge brings it.
    #[test]
    fn a_hedged_nodes_cap_or_gap_never_beats_the_preferred_nodes_answer() {
        const LOGS: u32 = 424_248;
        const SIMULATE: u32 = 424_249;
        const REVERT: u32 = 424_250;
        const SLOW: Duration = Duration::from_millis(2_500);
        const LOGS_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"result":[{"logIndex":"0x0"}]}"#;
        const CAPPED: &str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"block range exceeded: maximum is 500"}}"#;
        const MISSING: &str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_simulateV1 does not exist/is not available"}}"#;
        const REVERTED: &str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":3,"message":"execution reverted","data":"0x"}}"#;

        storage::tests::with_temp_state("pool-hedge-early", || {
            let (logs, _) = scripted_node(SLOW, Some(http("200 OK", LOGS_BODY)));
            let (capped, capped_log) = scripted_node(Duration::ZERO, Some(http("200 OK", CAPPED)));
            seed_pool(
                LOGS,
                &[(&logs, RpcSource::User), (&capped, RpcSource::Public)],
            );
            let (simulates, _) = scripted_node(SLOW, Some(http("200 OK", ANSWER)));
            let (missing, missing_log) =
                scripted_node(Duration::ZERO, Some(http("200 OK", MISSING)));
            seed_pool(
                SIMULATE,
                &[(&simulates, RpcSource::User), (&missing, RpcSource::Public)],
            );
            let (dead, _) = scripted_node(Duration::from_secs(3), None);
            let (reverts, _) = scripted_node(Duration::ZERO, Some(http("200 OK", REVERTED)));
            seed_pool(
                REVERT,
                &[(&dead, RpcSource::User), (&reverts, RpcSource::Public)],
            );

            let read = |chain_id: u32, method: &'static str| {
                std::thread::spawn(move || {
                    let began = Instant::now();
                    let answer = call(chain_id, method, json!([]));
                    (answer, began.elapsed())
                })
            };
            let joined = |reader: std::thread::JoinHandle<(Result<Value, PoolError>, Duration)>| {
                reader
                    .join()
                    .unwrap_or_else(|_| unreachable!("a reader panicked"))
            };
            let (logs_read, simulate_read, revert_read) = (
                read(LOGS, "eth_getLogs"),
                read(SIMULATE, "eth_simulateV1"),
                read(REVERT, "eth_call"),
            );

            let (answer, waited) = joined(logs_read);
            assert_eq!(
                result_of(&answer),
                Some(json!([{ "logIndex": "0x0" }])),
                "a public node's range cap beat the logs: {answer:?}"
            );
            assert!(waited >= SLOW, "{waited:?}");

            let (answer, waited) = joined(simulate_read);
            assert_eq!(
                result_of(&answer),
                Some(json!("0x2a")),
                "a node without the method beat the answer: {answer:?}"
            );
            assert!(waited >= SLOW, "{waited:?}");

            let (answer, waited) = joined(revert_read);
            assert_eq!(
                answer
                    .as_ref()
                    .ok()
                    .and_then(|body| body.pointer("/error/code").cloned()),
                Some(json!(3)),
                "{answer:?}"
            );
            assert!(
                waited < HEDGE_AFTER + Duration::from_millis(900),
                "a revert waited for the dead node: {waited:?}"
            );

            // Each hedged node was asked once, and every call closed.
            assert_eq!(arrivals(&capped_log).len(), 1);
            assert_eq!(arrivals(&missing_log).len(), 1);
            assert_eq!(settle(Duration::from_secs(3)), 0);
        });
    }

    /// 083 H6: when the core's own post answers first, that is the caller's
    /// answer, and the hedge that lands after it is dropped — no second
    /// reply, nothing reopened.
    #[test]
    fn the_cores_own_answer_wins_and_the_late_hedge_is_dropped() {
        const CHAIN: u32 = 424_251;

        storage::tests::with_temp_state("pool-hedge-late", || {
            let (first, first_log) = scripted_node(
                Duration::from_millis(2_000),
                Some(http("200 OK", &result_body("0xa"))),
            );
            let (second, second_log) = scripted_node(
                Duration::from_millis(1_500),
                Some(http("200 OK", &result_body("0xb"))),
            );
            seed_pool(
                CHAIN,
                &[(&first, RpcSource::User), (&second, RpcSource::Public)],
            );

            let began = Instant::now();
            let answer = call(CHAIN, "eth_call", json!([]));
            let waited = began.elapsed();
            assert_eq!(result_of(&answer), Some(json!("0xa")), "{answer:?}");
            assert!(waited < Duration::from_millis(2_900), "{waited:?}");
            assert_eq!(arrivals(&second_log).len(), 1, "no hedge was sent");

            // The hedge lands at about 3 s, into a call that is gone.
            std::thread::sleep(Duration::from_millis(3_400).saturating_sub(began.elapsed()));
            assert_eq!(open_calls(), 0, "the late hedge reopened the call");
            assert_eq!(arrivals(&first_log).len(), 1);
            assert_eq!(arrivals(&second_log).len(), 1);
        });
    }

    /// 083 H6: hedges follow one another down a longer pool. A swallows the
    /// read; B, hedged at 1.5 s, fails half a second later; C is hedged the
    /// moment B fails, and answers. Each node is asked once, and the core,
    /// told both failures in its own order, starts the next read at C.
    #[test]
    fn hedges_follow_one_another_down_a_three_node_pool() {
        const CHAIN: u32 = 424_252;

        storage::tests::with_temp_state("pool-hedge-three", || {
            let (a, a_log) = scripted_node(Duration::from_secs(4), None);
            let (b, b_log) = scripted_node(Duration::from_millis(500), None);
            let (c, c_log) =
                scripted_node(Duration::ZERO, Some(http("200 OK", &result_body("0xc"))));
            seed_pool(
                CHAIN,
                &[
                    (&a, RpcSource::User),
                    (&b, RpcSource::Default),
                    (&c, RpcSource::Public),
                ],
            );

            let began = Instant::now();
            let answer = call(CHAIN, "eth_call", json!([]));
            let waited = began.elapsed();
            assert_eq!(result_of(&answer), Some(json!("0xc")), "{answer:?}");
            assert!(
                waited >= HEDGE_AFTER + Duration::from_millis(500),
                "{waited:?}"
            );
            assert!(
                waited < HEDGE_AFTER + Duration::from_millis(1_400),
                "{waited:?}"
            );
            assert_eq!(settle(Duration::from_secs(4)), 0, "the read was left open");
            for log in [&a_log, &b_log, &c_log] {
                assert_eq!(arrivals(log).len(), 1, "a node was asked twice");
            }

            let again = Instant::now();
            let answer = call(CHAIN, "eth_call", json!([]));
            assert_eq!(result_of(&answer), Some(json!("0xc")), "{answer:?}");
            assert!(
                again.elapsed() < HEDGE_AFTER,
                "the core never learned A and B failed"
            );
            assert_eq!(arrivals(&a_log).len(), 1);
            assert_eq!(arrivals(&b_log).len(), 1);
        });
    }

    /// 083 H6, review finding 3: a hedge never costs the core's own post
    /// its worker. Hedges spend their own budget — with every hedge slot
    /// taken a hedge is refused and the core's post still gets a worker —
    /// and none is sent at all once the core's own workers are down to
    /// their reserve, so the core's next post never runs on the pool thread.
    #[test]
    fn a_hedge_never_takes_a_worker_the_cores_own_post_needs() {
        use std::sync::atomic::Ordering;

        storage::tests::with_temp_state("pool-hedge-budget", || {
            let (tx, rx) = channel();
            let work =
                || -> Work { Box::new(|| Message::Ask(Request::Refresh { chain_id: None })) };
            // Relative, so a worker still finishing from another test is
            // counted rather than overwritten.
            HEDGES.fetch_add(MAX_HEDGES, Ordering::Relaxed);
            let hedged = spawn_hedge(&tx, work());
            let posted = spawn_worker(&tx, work(), &WORKERS, MAX_WORKERS);
            HEDGES.fetch_sub(MAX_HEDGES, Ordering::Relaxed);
            assert!(!hedged, "a hedge past its budget was sent");
            assert!(posted, "the core's post lost its worker to the hedges");
            assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());

            // The core's workers busy down to their reserve: no hedge, though
            // the hedges' own budget is untouched.
            let busy = MAX_WORKERS - WORKER_RESERVE;
            WORKERS.fetch_add(busy, Ordering::Relaxed);
            let hedged = spawn_hedge(&tx, work());
            let posted = spawn_worker(&tx, work(), &WORKERS, MAX_WORKERS);
            WORKERS.fetch_sub(busy, Ordering::Relaxed);
            assert!(!hedged, "a hedge ate into the core's reserve");
            assert!(posted, "the reserve was not there for the core's post");
            assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());

            // With room again, a hedge goes out.
            assert!(spawn_hedge(&tx, work()), "no hedge with every worker free");
            assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
        });
    }

    /// 083 H6, review finding 5: the timeout path, as W13's black hole
    /// really behaves — the first node accepts the read and never answers,
    /// so the core's own post runs into its full read timeout. The caller
    /// has the hedge's answer at `HEDGE_AFTER`; the core, told the timeout
    /// when it happens, takes the hedge's outcome for the second node rather
    /// than asking again, and closes the call. The next read goes to the
    /// second node first and waits for no hedge.
    #[test]
    fn a_node_that_never_answers_times_out_behind_the_hedge() {
        const CHAIN: u32 = 424_253;
        let timeout = Duration::from_millis(u64::from(RPC_READ_TIMEOUT_MS));

        storage::tests::with_temp_state("pool-hedge-timeout", || {
            let (dead, dead_log) = scripted_node(timeout + Duration::from_secs(5), None);
            let (live, live_log) =
                scripted_node(Duration::ZERO, Some(http("200 OK", &result_body("0xb"))));
            seed_pool(
                CHAIN,
                &[(&dead, RpcSource::User), (&live, RpcSource::Public)],
            );

            let began = Instant::now();
            let answer = call(CHAIN, "eth_call", json!([]));
            let waited = began.elapsed();
            assert_eq!(result_of(&answer), Some(json!("0xb")), "{answer:?}");
            assert!(
                waited >= HEDGE_AFTER && waited < HEDGE_AFTER + Duration::from_secs(1),
                "{waited:?}"
            );

            // Still open while the core waits out the first node...
            assert_eq!(open_calls(), 1, "the core stopped waiting early");
            // ...and closed once its read timeout has passed.
            let left = timeout.saturating_sub(began.elapsed()) + Duration::from_secs(3);
            assert_eq!(settle(left), 0, "the timed-out read was left open");
            assert!(began.elapsed() >= timeout, "closed before the timeout");
            assert_eq!(arrivals(&dead_log).len(), 1);
            assert_eq!(
                arrivals(&live_log).len(),
                1,
                "the second node was asked twice"
            );

            let again = Instant::now();
            let answer = call(CHAIN, "eth_call", json!([]));
            assert_eq!(result_of(&answer), Some(json!("0xb")), "{answer:?}");
            assert!(
                again.elapsed() < HEDGE_AFTER,
                "the core never learned the first node timed out"
            );
            assert_eq!(arrivals(&dead_log).len(), 1);
        });
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
