//! The app's one HTTP agent, and how it gets out of the machine.
//!
//! **Everything that talks to the network goes through [`agent`].** Not by
//! convention — there is exactly one place an agent is constructed, and it is
//! this one, so a caller cannot get a client that skipped the proxy by
//! forgetting a line. That mattered less when the registry was the only network
//! consumer and it was true by accident; it stops being an accident here,
//! before feature 020's tunnel client and the bug reporter arrive.
//!
//! Two things go wrong on a desktop that `ureq`'s own proxy handling does not
//! cover, and both of them present identically: every registry call sits until
//! its budget elapses and the flow ends on a timeout sheet blaming the index
//! service, while the person's browser loads the same host fine.
//!
//! ## 1. A SOCKS5 proxy is asked to connect to an address we cannot look up
//!
//! `ureq` maps `socks://` and `socks5://` to [`ProxyProtocol::Socks5`], whose
//! `resolve_target` default is `true` — the target hostname is resolved
//! LOCALLY and only the resulting address is handed to the proxy. That is a
//! fine default for a proxy that exists to change the route. It is exactly
//! wrong for the proxy this application actually meets, which exists because
//! name resolution on the machine does not work: the local lookup is precisely
//! the step that cannot succeed, so the request never even reaches the SOCKS
//! handshake. `curl` spells the working variant `socks5h`, browsers do it by
//! default, and there is no case where this app wants the other one — so a
//! SOCKS5 proxy is switched to resolving at the far end. It also stops the
//! lookup leaking around a proxy that was configured to carry it.
//!
//! Note the environment does not have to name `socks` for this to bite:
//! `ureq` reads `ALL_PROXY` BEFORE `HTTPS_PROXY`, so a session exporting both
//! — which is what every proxy tool's setup snippet emits — gets the SOCKS one.
//!
//! ## 2. A desktop launch has no proxy environment at all
//!
//! `ureq` finds a proxy in `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY`. That is
//! right for a CLI, which is started from a shell that has them, and wrong for
//! a desktop application: `Exec=vela-wallet` in the .desktop file inherits the
//! systemd user environment, and a proxy exported from `~/.bashrc` is not in
//! it. So when the environment says nothing, the desktop's own setting is read
//! — the one the browser obeys.
//!
//! ### Per platform
//!
//! * **Linux** — GNOME's `org.gnome.system.proxy`, through `gsettings`. It is
//!   the setting the GNOME/GTK network stack itself uses, so honouring it makes
//!   this app agree with the rest of the session. Read once per process: the
//!   value cannot change mid-flight in any way worth a subprocess per request.
//! * **Windows** — `ureq`'s own `win-system-proxy` feature reads the WinINET
//!   registry keys inside `Proxy::try_from_env()`. Nothing to do here.
//! * **macOS** — `scutil --proxy`, the same dictionary System Settings →
//!   Network → Proxies writes. An earlier version of this module read
//!   nothing here on the premise that a macOS system proxy is "installed
//!   into the network stack" — true of a TUN-mode tool, false of the
//!   ordinary HTTP/SOCKS proxy in that pane, which only CFNetwork clients
//!   honour and a Rust socket walks straight past (spec 038 finding 10).
//!
//! ## 3. The environment is not the most trustworthy source, and no answer
//!      is good forever
//!
//! A GUI app's environment is whatever launched it — Finder gives one, a
//! terminal another — so a proxy exported in a shell is the LEAST stable
//! statement about how this machine gets out, and it was being trusted first
//! and cached for the life of the process (spec 038 finding 11). The dead
//! `all_proxy=socks5://127.0.0.1:1080` in a developer's shell is how "the
//! Passkey Index service is unreachable" appeared under a healthy index.
//!
//! So this module now holds a short list of CANDIDATES, in order of trust —
//! the system setting, the environment, direct — and [`with_candidates_for`]
//! walks it: a request whose failure is a transport failure is retried once
//! on the next candidate, and only when the last one refuses is the failure
//! reported, with the one bit the screen needs: did it fail to get out of
//! THIS MACHINE (`local`), or did a route exist and the far end not answer?
//! The list is re-derived at a dead end and every [`REDERIVE_AFTER`], so
//! a proxy that comes back or a setting that changes is honoured without a
//! restart. Contract: `specs/038-first-run-parity/contracts/proxy-candidates.md`.
//!
//! Where a request starts on that list is remembered per HOST (spec 083 W13).
//! It used to be one index for the whole process, moved by any failure: one
//! slow site through the proxy sent the wallet's RPC, relay and prices Direct
//! for a minute — from behind a firewall, the route that does not work. Now a
//! failure moves only its host, and a success pins the route that carried
//! it, which that host keeps even when everyone else moves on. Everyone's
//! route moves past a proxy only on evidence that every host would fail on
//! it: the proxy refused the connection or could not be reached, or three
//! hosts had to walk past it to a route that carried them with none carried
//! by it in between (a proxy that accepts and reaches nothing). A host on
//! the proxy's bypass list never went through it — `NO_PROXY`, and on
//! Windows the `ProxyOverride` list `ureq` reads with the WinINET proxy — so
//! its failures say nothing about the proxy. That shared route is also the
//! one [`agent`] hands to callers that cannot walk. What was learned lasts
//! until the list is re-derived.
//!
//! What is deliberately NOT handled: `mode = 'auto'` (a PAC URL is a JavaScript
//! program, and running one to reach the key registry is not a trade this app
//! makes) and KDE's `kioslaverc`. Both fall through to no proxy, which is the
//! behaviour before this module existed.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use ureq::http::Uri;
use ureq::{Agent, Proxy, ProxyProtocol};

/// The app's HTTP agent.
///
/// Connection reuse matters: the publish path makes a challenge call, a
/// register call and then polls a task every two seconds, and a fresh TLS
/// handshake for each of those is most of the wall clock.
pub fn agent(timeout: Duration) -> Agent {
    // The current candidate, not `ureq`'s own pick: a SOCKS5 proxy from the
    // environment is asked to resolve the target LOCALLY, which is the one
    // step that cannot work on the machines that need a proxy most, and a
    // dead proxy in the environment must not be the last word (module note).
    agent_over(current().proxy(), timeout)
}

/// The host a url names, without its port or an IPv6 literal's brackets.
fn host_of(url: &str) -> &str {
    let host = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    // Strip the port, and the brackets an IPv6 literal carries.
    let host = host.rsplit_once(':').map_or(host, |(head, tail)| {
        if tail.chars().all(|c| c.is_ascii_digit()) {
            head
        } else {
            host
        }
    });
    host.trim_start_matches('[').trim_end_matches(']')
}

/// Is this url on this machine (or its own network's name for it)?
fn is_local(url: &str) -> bool {
    let host = host_of(url);
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(std::net::IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

/// How long a derived candidate list is trusted before `scutil` / the
/// environment are consulted again. Short enough that a proxy toggled in
/// System Settings is noticed; long enough that a burst of requests does not
/// spawn a subprocess each.
const REDERIVE_AFTER: Duration = Duration::from_secs(60);

/// One way out of the machine, in order of trust.
#[derive(Clone, Debug)]
enum Candidate {
    /// The desktop's own setting — what the browser next to us obeys.
    System(Proxy),
    /// `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY`, rewritten to resolve at the
    /// far end where that matters.
    Env(Proxy),
    /// No proxy at all. Always last, always present.
    Direct,
}

impl Candidate {
    fn proxy(&self) -> Option<&Proxy> {
        match self {
            Candidate::System(proxy) | Candidate::Env(proxy) => Some(proxy),
            Candidate::Direct => None,
        }
    }

    fn same_route(&self, other: &Self) -> bool {
        match (self.proxy(), other.proxy()) {
            (Some(a), Some(b)) => {
                a.host() == b.host() && a.port() == b.port() && a.protocol() == b.protocol()
            }
            (None, None) => true,
            _ => false,
        }
    }
}

struct Candidates {
    list: Vec<Candidate>,
    /// Where a host with nothing learned starts, and the route [`agent`]
    /// hands out: the first route not down for everyone ([`Route::down`]).
    current: usize,
    /// 083 W13: what each route of `list` has shown about itself, by index.
    routes: Vec<Route>,
    /// 083 W13: the route each host last got through on, or was moved to.
    /// Learned under this derivation only, so a re-derive starts every host
    /// afresh and the map never holds more than a minute's hosts.
    hosts: HashMap<String, usize>,
    derived_at: Instant,
    /// Which derivation this is: a request that started on an older list
    /// must not write what it learned into a newer one.
    generation: u64,
}

/// What one route has shown under this derivation (083 W13).
#[derive(Default)]
struct Route {
    /// The proxy itself would not take a connection, or could not be
    /// reached ([`route_is_dead`]): every host would fail on it, so every
    /// host skips it, including one whose own route it was.
    dead: bool,
    /// Hosts that failed through this proxy and then got through on a later
    /// route, since the last request this proxy carried. Only a host that
    /// another route reached counts: a dead dApp fails every route and says
    /// nothing about the proxy (083 W13 review).
    walked_past: HashSet<String>,
}

/// How many hosts must have walked past a proxy, with none carried by it in
/// between, before everyone does. One is a slow site, which must never move
/// the wallet's other hosts (083 W13, D3). Three with no success between is
/// a proxy that accepts and reaches nothing — its upstream node is dead —
/// and [`agent`]'s callers and the caBLE tunnel, which cannot walk, would
/// otherwise sit on it until the list is re-derived, and again after.
const WALKED_PAST_BY: usize = 3;

impl Route {
    fn down(&self) -> bool {
        self.dead || self.walked_past.len() >= WALKED_PAST_BY
    }
}

impl Candidates {
    fn of(list: Vec<Candidate>) -> Self {
        static GENERATIONS: AtomicU64 = AtomicU64::new(0);
        Candidates {
            routes: list.iter().map(|_| Route::default()).collect(),
            list,
            current: 0,
            hosts: HashMap::new(),
            derived_at: Instant::now(),
            generation: GENERATIONS.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Everyone's route: the first one not down. Recomputed from what each
    /// route has shown rather than stepped, so a route that failed further
    /// down the list can never push everyone past an earlier one that still
    /// works (083 W13 review). Direct is never down: it has no proxy to die
    /// and nothing after it to walk to.
    fn settle(&mut self) {
        let last = self.list.len().saturating_sub(1);
        self.current = self
            .routes
            .iter()
            .take(last)
            .position(|route| !route.down())
            .unwrap_or(last);
    }
}

fn state() -> &'static Mutex<Option<Candidates>> {
    static STATE: OnceLock<Mutex<Option<Candidates>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

/// System → environment → direct, duplicates collapsed, direct always last.
fn derive_candidates() -> Vec<Candidate> {
    let mut list = Vec::with_capacity(3);
    if let Some(system) = desktop_proxy() {
        list.push(Candidate::System(system));
    }
    if let Some(from_env) = Proxy::try_from_env() {
        // The environment is the more specific statement about the ROUTE and
        // is left to stand — except for the one detail it cannot express
        // (see `resolve_at_the_proxy`). On Windows this call also consults
        // the registry (the `win-system-proxy` feature).
        let env = Candidate::Env(resolve_at_the_proxy(&from_env).unwrap_or(from_env));
        if !list.iter().any(|known| known.same_route(&env)) {
            list.push(env);
        }
    }
    list.push(Candidate::Direct);
    list
}

/// The list, derived afresh when there is none or it is older than
/// [`REDERIVE_AFTER`].
fn fresh(slot: &mut Option<Candidates>) -> &mut Candidates {
    if slot
        .as_ref()
        .is_some_and(|c| c.derived_at.elapsed() >= REDERIVE_AFTER)
    {
        *slot = None;
    }
    slot.get_or_insert_with(|| Candidates::of(derive_candidates()))
}

/// The candidate a request that names no host goes through right now.
fn current() -> Candidate {
    let Ok(mut guard) = state().lock() else {
        return Candidate::Direct;
    };
    let candidates = fresh(&mut guard);
    candidates
        .list
        .get(candidates.current)
        .cloned()
        .unwrap_or(Candidate::Direct)
}

/// What one request walks: its own copy of the list, so what other requests
/// do meanwhile cannot make it skip a route (083 W13).
struct Walk {
    list: Vec<Candidate>,
    /// Where this host starts: see [`begin`].
    start: usize,
    generation: u64,
}

/// A host that has learned a route starts there, even when everyone else has
/// moved on: its own evidence beats the crowd's, and other hosts' failures
/// must never reroute it (083 D3). A host with nothing learned starts where
/// everyone does. Either way a dead proxy is skipped — it fails every host.
fn begin(host: &str) -> Walk {
    let Ok(mut guard) = state().lock() else {
        return Walk {
            list: vec![Candidate::Direct],
            start: 0,
            generation: 0,
        };
    };
    let candidates = fresh(&mut guard);
    let last = candidates.list.len().saturating_sub(1);
    let from = candidates
        .hosts
        .get(host)
        .copied()
        .unwrap_or(candidates.current);
    Walk {
        start: (from..last)
            .find(|&at| candidates.routes.get(at).is_some_and(|route| !route.dead))
            .unwrap_or(last),
        list: candidates.list.clone(),
        generation: candidates.generation,
    }
}

/// Change the list a walk started on — or nothing, once it was re-derived.
fn learn(generation: u64, change: impl FnOnce(&mut Candidates)) {
    if let Ok(mut guard) = state().lock()
        && let Some(candidates) = guard.as_mut().filter(|c| c.generation == generation)
    {
        change(candidates);
    }
}

/// Route `at` failed for `host`: the host starts past it from now on, and
/// everyone skips it when the route itself is dead. `max`, not `+= 1`: two
/// requests failing on one route at once move one step, not two (083 W13).
fn step_past(generation: u64, host: &str, at: usize, route_is_dead: bool) {
    learn(generation, |candidates| {
        if route_is_dead && let Some(route) = candidates.routes.get_mut(at) {
            route.dead = true;
            candidates.settle();
        }
        let next = at + 1;
        if next >= candidates.list.len() {
            return;
        }
        let learned = candidates.hosts.entry(host.to_owned()).or_insert(next);
        *learned = (*learned).max(next);
    });
}

/// Route `at` carried `host`, after the proxies at `walked_past` failed it.
/// The host is pinned to `at`; a success through `at`'s proxy clears the
/// count against it; and each proxy walked past counts this host against
/// itself ([`WALKED_PAST_BY`]).
fn carried(generation: u64, host: &str, at: usize, through_proxy: bool, walked_past: &[usize]) {
    learn(generation, |candidates| {
        candidates.hosts.insert(host.to_owned(), at);
        if through_proxy && let Some(route) = candidates.routes.get_mut(at) {
            route.walked_past.clear();
        }
        for &past in walked_past {
            if let Some(route) = candidates.routes.get_mut(past) {
                route.walked_past.insert(host.to_owned());
            }
        }
        candidates.settle();
    });
}

/// Every route from the walk's start failed for `host`. The host forgets
/// what it learned, so its next request starts where everyone does. If
/// everyone's route had itself moved on, this is the dead end the list is
/// dropped for: the next request re-derives it, and a proxy that came back
/// is tried again.
fn exhausted(generation: u64, host: &str) {
    let Ok(mut guard) = state().lock() else {
        return;
    };
    match guard.as_mut() {
        Some(candidates) if candidates.generation == generation => {
            if candidates.current > 0 {
                *guard = None;
            } else {
                candidates.hosts.remove(host);
            }
        }
        _ => {}
    }
}

/// The proxy a request to `target` really goes through on `candidate`.
/// `None` for Direct, and for a target on the proxy's bypass list, which
/// `ureq` connects to directly (`run.rs`, `connect.rs`, `socks.rs`, each
/// asking [`Proxy::is_no_proxy`]). That list is `NO_PROXY`, GNOME's
/// ignore-hosts, macOS's exceptions — and on Windows the `ProxyOverride`
/// value `ureq` reads with the WinINET proxy, which v2rayN sets to
/// `<local>;localhost;127.*;10.*;172.16.*…;192.168.*` (083 W13 review).
/// `None` for `target` (no url to judge) counts as through the proxy.
fn through_proxy<'a>(candidate: &'a Candidate, target: Option<&Uri>) -> Option<&'a Proxy> {
    candidate
        .proxy()
        .filter(|proxy| target.is_none_or(|uri| !proxy.is_no_proxy(uri)))
}

/// A route every host would fail on: the proxy would not take the
/// connection, or could not be reached at all. Anything else — a timeout, a
/// proxy that took the request and could not reach this host, a reset, a
/// TLS failure — may be about the host alone (a dead dApp, a black-holed
/// node, a bad certificate), and moving everyone off a working proxy for it
/// is the 083 W13 failure.
///
/// A target on the bypass list never went through the proxy, so its refusal
/// is the target's: a LAN node that is down must not move the wallet off
/// the proxy on every poll (083 W13 review).
fn route_is_dead(candidate: &Candidate, target: Option<&Uri>, error: &ureq::Error) -> bool {
    let Some(proxy) = through_proxy(candidate, target) else {
        return false;
    };
    match error {
        // SOCKS replies about the target arrive as `Other` (`socks` v5.rs),
        // and an HTTP proxy's as `ConnectProxyFailed`: these kinds are the
        // connection to the proxy itself.
        ureq::Error::Io(io)
            if matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
                    | std::io::ErrorKind::NetworkDown
            ) =>
        {
            true
        }
        // A name that would not resolve is the proxy's own only when the
        // proxy resolves the target; SOCKS4 has it looked up here first.
        _ => !proxy.resolve_target() && failed_inside_this_machine(error),
    }
}

/// The proxy to configure on an agent right now, or `None` for direct.
///
/// Kept as the module's one public read of the decision (the caBLE tunnel
/// dial logs it); everything else should go through [`with_candidates_for`]
/// so a refused route is retried rather than reported.
pub fn system_proxy() -> Option<Proxy> {
    current().proxy().cloned()
}

/// A transport failure, after every candidate was tried.
#[derive(Debug)]
pub struct Transport {
    /// The most telling of the routes' errors, verbatim (see [`Failure`]).
    pub error: ureq::Error,
    /// The direct attempt could not even resolve or route to the host —
    /// judged on that attempt alone, whatever `error` is, as before 083: an
    /// offline laptop whose proxy answers `502` or hangs up is still "your
    /// network", not the index service being down (spec 038). `false` means
    /// a route existed and the far end did not answer — which may still be
    /// the person's network, but is not something a different proxy fixes.
    pub local: bool,
}

/// Is this the kind of error that a different route might cure?
///
/// Everything that is not "the server answered" (`StatusCode`) or "the body
/// the server sent was unusable" is a transport failure for this purpose —
/// including a TLS failure, which is what a proxy that intercepts looks like.
fn is_transport(error: &ureq::Error) -> bool {
    !matches!(
        error,
        ureq::Error::StatusCode(_)
            | ureq::Error::Json(_)
            | ureq::Error::BodyExceedsLimit(_)
            | ureq::Error::Decompress(..)
            | ureq::Error::BodyStalled
    )
}

/// A direct attempt that failed before any packet could have reached the
/// host: nothing to resolve the name with, or no route to it.
fn failed_inside_this_machine(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::HostNotFound => true,
        ureq::Error::Io(io) => {
            matches!(
                io.kind(),
                std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
                    | std::io::ErrorKind::NetworkDown
            ) || resolver_code_said_no(io)
                || resolver_said_no(&io.to_string())
        }
        _ => false,
    }
}

/// Windows' resolver speaks the system's language — "不知道这样的主机。"
/// on the 083 device, not "No such host is known" — so its sentence cannot
/// be matched; its Winsock code can. Without this, every failed lookup on a
/// non-English Windows read as "the far end did not answer", and an offline
/// laptop was told the index service was down (083 W13 review).
/// `WSAHOST_NOT_FOUND` (11001) and `WSANO_DATA` (11004).
#[cfg(windows)]
fn resolver_code_said_no(io: &std::io::Error) -> bool {
    matches!(io.raw_os_error(), Some(11001 | 11004))
}

/// Elsewhere `getaddrinfo` failures carry no OS code; the sentence is read.
#[cfg(not(windows))]
fn resolver_code_said_no(_io: &std::io::Error) -> bool {
    false
}

/// A resolver failure does not arrive as `HostNotFound` here: `std` surfaces
/// `getaddrinfo` as an `Uncategorized` io error carrying the libc sentence.
/// These are the three desktops' words for "I could not even look the name
/// up" — macOS / BSD, glibc, and Windows — matched on the stable fragment.
fn resolver_said_no(message: &str) -> bool {
    message.contains("failed to lookup address information")
        || message.contains("nodename nor servname")
        || message.contains("Name or service not known")
        || message.contains("No such host is known")
        || message.contains("No address associated with hostname")
}

/// One route's failure, kept while the walk goes on.
struct Failure {
    error: ureq::Error,
    direct: bool,
    /// The proxy would not take the connection ([`route_is_dead`]).
    route_is_dead: bool,
}

impl Failure {
    /// How much this failure says about the far end, for choosing the one a
    /// caller hears when every route failed (083 W13). Only the last route's
    /// error used to be kept, so a sponsorship that timed out through the
    /// proxy — and may have been paid — read as `network_error` because
    /// Direct then failed a lookup.
    ///
    /// 2: a timeout — the request may have arrived, the one fact a caller
    /// must not lose. 1: the far end, or a proxy on its behalf, answered with
    /// a failure. 0: it never left this machine (a proxy that refused, a
    /// name nothing resolved) — says nothing about the host.
    fn weight(&self) -> u8 {
        if matches!(self.error, ureq::Error::Timeout(_)) {
            2
        } else if self.route_is_dead || failed_inside_this_machine(&self.error) {
            0
        } else {
            1
        }
    }
}

/// What a walk heard from the routes it tried, for the one answer its
/// caller gets when none carried the request.
#[derive(Default)]
struct Heard {
    /// The earlier failure stands unless a later one says more — first wins
    /// among equals, so the route the person's setup intends speaks. Among
    /// failures that never left the machine the LAST stands: the direct
    /// attempt's words, as before 083.
    kept: Option<Failure>,
    /// [`Transport::local`], from the direct attempt only. Choosing `kept`
    /// by weight must not change it: before this was split out, a proxy's
    /// `502` outranked Direct's "no such host" and an offline laptop read
    /// as the index being down (083 W13 review).
    direct_local: bool,
}

impl Heard {
    fn failed(&mut self, failure: Failure) {
        if failure.direct {
            self.direct_local = failed_inside_this_machine(&failure.error);
        }
        self.kept = Some(match self.kept.take() {
            Some(kept) if kept.weight() > 0 && kept.weight() >= failure.weight() => kept,
            _ => failure,
        });
    }

    fn into_transport(self) -> Transport {
        Transport {
            // Not `None` after a walk: the list always holds Direct and the
            // walk's start is on it.
            error: self
                .kept
                .map_or(ureq::Error::ConnectionFailed, |kept| kept.error),
            local: self.direct_local,
        }
    }
}

/// Run one request to `url` over the candidate chain.
///
/// `call` is invoked with an agent for the first candidate worth trying; a
/// transport failure moves to the next candidate and calls it again, once
/// per candidate. The first `Ok` — or the first non-transport `Err`, which
/// is the server talking — is returned as-is.
///
/// A loopback target never takes a proxy, so it runs direct and once;
/// everything else walks the chain from the route that last worked for its
/// host (083 W13). Every walking caller names its url — the pool's
/// per-endpoint calls (T028), the registry, the relay — so one host's
/// slowness never reroutes another, and the proxy's bypass list is judged
/// against the real target.
pub fn with_candidates_for<T>(
    url: &str,
    timeout: Duration,
    mut call: impl FnMut(&Agent) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    if is_local(url) {
        return call(&agent_over(None, timeout)).map_err(|error| Transport {
            local: failed_inside_this_machine(&error),
            error,
        });
    }
    let target = url.parse::<Uri>().ok();
    walk(
        &host_of(url).to_ascii_lowercase(),
        target.as_ref(),
        timeout,
        call,
    )
}

/// The walk behind [`with_candidates_for`]: from `host`'s start (see
/// [`begin`]), on this request's own copy of the list (083 W13).
fn walk<T>(
    host: &str,
    target: Option<&Uri>,
    timeout: Duration,
    mut call: impl FnMut(&Agent) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    let Walk {
        list,
        start,
        generation,
    } = begin(host);
    let mut heard = Heard::default();
    // The proxies this request failed through, not dead, before a route
    // carried it: evidence against each, counted only if one does.
    let mut walked_past = Vec::new();
    for (at, candidate) in list.iter().enumerate().skip(start) {
        let through_proxy = through_proxy(candidate, target).is_some();
        match call(&agent_over(candidate.proxy(), timeout)) {
            Ok(value) => {
                carried(generation, host, at, through_proxy, &walked_past);
                return Ok(value);
            }
            Err(error) if is_transport(&error) => {
                let failure = Failure {
                    route_is_dead: route_is_dead(candidate, target, &error),
                    direct: candidate.proxy().is_none(),
                    error,
                };
                step_past(generation, host, at, failure.route_is_dead);
                if through_proxy && !failure.route_is_dead {
                    walked_past.push(at);
                }
                heard.failed(failure);
            }
            Err(error) => {
                return Err(Transport {
                    error,
                    local: false,
                });
            }
        }
    }
    exhausted(generation, host);
    Err(heard.into_transport())
}

/// One agent per way out and timeout, kept for the life of the process.
///
/// A `ureq` agent IS its connection pool. This built a fresh one for every
/// request, so nothing was ever reused: each RPC call, index fetch and rate
/// read paid DNS, TCP and a TLS handshake of its own — three of them per
/// chain on a balance read, where the browser beside it keeps its sockets
/// alive (the 078 loading audit). Agents are cheap handles over a shared
/// pool, so the cache hands out clones.
fn agent_over(proxy: Option<&Proxy>, timeout: Duration) -> Agent {
    static AGENTS: OnceLock<Mutex<std::collections::HashMap<String, Agent>>> = OnceLock::new();
    let key = format!("{proxy:?}|{}", timeout.as_millis());
    let agents = AGENTS.get_or_init(Mutex::default);
    if let Ok(agents) = agents.lock()
        && let Some(agent) = agents.get(&key)
    {
        return agent.clone();
    }
    let mut config = Agent::config_builder()
        .timeout_global(Some(timeout))
        // A balance read talks to two dozen chains' endpoints at once; the
        // default ten idle sockets would drop most of them between calls.
        .max_idle_connections(128)
        .max_idle_connections_per_host(8);
    // `None` here means `ureq`'s own answer stands, which for a `Direct`
    // candidate must be "no proxy" even if the environment names one — that
    // is the whole point of the candidate.
    config = config.proxy(proxy.cloned());
    let agent = config.build().new_agent();
    if let Ok(mut agents) = agents.lock() {
        agents.insert(key, agent.clone());
    }
    agent
}

/// The same proxy, with the target hostname resolved at the far end.
///
/// `None` when there is nothing to change: an HTTP `CONNECT` proxy already
/// resolves remotely, and so does `socks4a` / `socks5h`, where the person
/// spelled the intent out. SOCKS4 is left alone as well — the protocol has no
/// way to carry a hostname, so "resolve remotely" is not a thing a SOCKS4
/// server can be asked for, and forcing it would trade a slow failure for an
/// immediate one.
fn resolve_at_the_proxy(proxy: &Proxy) -> Option<Proxy> {
    if proxy.protocol() != ProxyProtocol::Socks5 || !proxy.resolve_target() {
        return None;
    }

    // Rebuilt rather than mutated: `Proxy` is immutable, and `resolve_target`
    // is the single flag both connectors read (`socks.rs`, `connect.rs`), so
    // carrying everything else across verbatim is the whole job.
    let mut builder = Proxy::builder(ProxyProtocol::Socks5)
        .host(proxy.host())
        .port(proxy.port())
        .resolve_target(false);
    if let Some(username) = proxy.username() {
        builder = builder.username(username);
        if let Some(password) = proxy.password() {
            builder = builder.password(password);
        }
    }
    // `Proxy` can answer `is_no_proxy` but cannot list its entries, so the
    // bypass list is re-read from where `ureq` read it. Losing it would send a
    // localhost endpoint — which is what a self-hosted registry looks like —
    // out through the proxy and back, if it came back at all.
    for entry in no_proxy_from_env() {
        builder = builder.no_proxy(&entry);
    }

    builder.build().ok()
}

/// `NO_PROXY` / `no_proxy`, split the way `ureq` splits it.
fn no_proxy_from_env() -> Vec<String> {
    ["NO_PROXY", "no_proxy"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| {
            value
                .split(',')
                .map(|entry| entry.trim().to_owned())
                .filter(|entry| !entry.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn desktop_proxy() -> Option<Proxy> {
    // Windows: `ureq`'s `win-system-proxy` feature reads WinINET inside
    // `Proxy::try_from_env()`, so the system setting arrives as the `Env`
    // candidate there.
    None
}

/// The macOS system proxy, from the dictionary `scutil --proxy` prints — the
/// one System Settings → Network → Proxies writes and the browser obeys.
#[cfg(target_os = "macos")]
fn desktop_proxy() -> Option<Proxy> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    proxy_from_scutil(&text)
}

/// Parse `scutil --proxy` output. Separate from the subprocess so the
/// captured dictionary in `research.md` is a unit test.
///
/// HTTPS first: every URL this client builds is https. `SOCKS` last and as
/// `socks5h`, for the reason in the module note. An entry is only an entry
/// when its `*Enable` is 1 and its port is non-zero — a host left over from
/// a disabled setting is not a route.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn proxy_from_scutil(text: &str) -> Option<Proxy> {
    let mut scalars = std::collections::HashMap::new();
    let mut exceptions = Vec::new();
    let mut in_exceptions = false;
    for line in text.lines() {
        let line = line.trim();
        if in_exceptions {
            if line.starts_with('}') {
                in_exceptions = false;
            } else if let Some((_, value)) = line.split_once(" : ") {
                exceptions.push(value.trim().to_owned());
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(" : ") {
            if key == "ExceptionsList" {
                in_exceptions = true;
            } else {
                scalars.insert(key.to_owned(), value.trim().to_owned());
            }
        }
    }
    let enabled = |name: &str| scalars.get(name).map(String::as_str) == Some("1");
    let (scheme, host, port) = ["HTTPS", "HTTP", "SOCKS"].into_iter().find_map(|scheme| {
        if !enabled(&format!("{scheme}Enable")) {
            return None;
        }
        let host = scalars.get(&format!("{scheme}Proxy"))?;
        let port: u16 = scalars.get(&format!("{scheme}Port"))?.parse().ok()?;
        (!host.is_empty() && port != 0).then(|| (scheme, host.clone(), port))
    })?;
    let mut builder = Proxy::builder(match scheme {
        "SOCKS" => ProxyProtocol::Socks5h,
        _ => ProxyProtocol::Http,
    })
    .host(&host)
    .port(port)
    .resolve_target(false);
    for entry in exceptions {
        builder = builder.no_proxy(&entry);
    }
    builder.build().ok()
}

/// GNOME's proxy setting, as a `ureq::Proxy`.
#[cfg(target_os = "linux")]
fn desktop_proxy() -> Option<Proxy> {
    if gsettings("org.gnome.system.proxy", "mode")? != "manual" {
        return None;
    }

    // HTTPS first: every URL this client builds is https, and a session that
    // configures the two differently means the https one. `socks` is the last
    // resort rather than the first because an HTTP CONNECT proxy is what the
    // http/https keys describe, and reading a socks port as one would produce a
    // connection that fails in a way nothing here could explain.
    let (scheme, host, port) = ["https", "http", "socks"].into_iter().find_map(|scheme| {
        let host = gsettings(&format!("org.gnome.system.proxy.{scheme}"), "host")?;
        let port: u16 = gsettings(&format!("org.gnome.system.proxy.{scheme}"), "port")?
            .parse()
            .ok()?;
        // A host set with the port left at 0 is a half-configured setting,
        // not an endpoint. Skip to the next scheme rather than build a URI
        // that cannot connect.
        (!host.is_empty() && port != 0).then_some((scheme, host, port))
    })?;

    let mut builder = Proxy::builder(match scheme {
        // Socks5h, for the reason in the module note: the machine that needs
        // this setting is usually the machine whose resolver does not work.
        "socks" => ProxyProtocol::Socks5h,
        _ => ProxyProtocol::Http,
    })
    .host(&host)
    .port(port)
    .resolve_target(false);

    // GNOME's `ignore-hosts` is the same idea as `NO_PROXY`.
    for entry in gsettings("org.gnome.system.proxy", "ignore-hosts")
        .as_deref()
        .map(parse_gvariant_list)
        .unwrap_or_default()
    {
        builder = builder.no_proxy(&entry);
    }

    builder.build().ok()
}

/// One `gsettings get`, with the GVariant quoting stripped off a string value.
///
/// `None` for every failure — a session with no `gsettings` on `PATH`, a schema
/// this GNOME does not ship, a key that is not there. Not finding a proxy is
/// the normal case, so none of those are worth a message.
#[cfg(target_os = "linux")]
fn gsettings(schema: &str, key: &str) -> Option<String> {
    let output = std::process::Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    Some(unquote(value.trim()).to_owned())
}

/// `'manual'` → `manual`. Leaves anything unquoted (an integer, a list) alone.
#[cfg(target_os = "linux")]
fn unquote(value: &str) -> &str {
    value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .unwrap_or(value)
}

/// `['localhost', '127.0.0.0/8']` → the two strings.
///
/// A hand-rolled reader for the one array shape this module asks for, rather
/// than a GVariant parser: the elements are hostnames and CIDR blocks, so
/// nothing here has to survive an escaped quote.
#[cfg(target_os = "linux")]
fn parse_gvariant_list(value: &str) -> Vec<String> {
    value
        .trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or("")
        .split(',')
        .map(|entry| unquote(entry.trim()).trim().to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A proxy is a way to reach the outside, and this machine is not outside.
    ///
    /// A local node (`http://127.0.0.1:8545`) behind a system proxy was
    /// unreachable, and the pool read that as an endpoint that failed and
    /// banned it. It also made four pool tests go red the moment this
    /// session's own proxy started refusing, without a line of their code
    /// changing.
    #[test]
    fn a_local_endpoint_is_never_proxied() {
        for url in [
            "http://127.0.0.1:8545",
            "http://127.0.0.1:8545/rpc?k=1",
            "https://localhost:8443/",
            "http://LOCALHOST:1234",
            "http://[::1]:8545",
            "http://foo.localhost/rpc",
        ] {
            assert!(is_local(url), "{url} was treated as remote");
        }
        for url in [
            "https://ethereum-rpc.publicnode.com",
            "https://rpc.gnosischain.com/",
            // Not loopback: a private LAN address still goes wherever the
            // machine's proxy settings say it goes.
            "http://192.168.1.10:8545",
            // A hostname that merely CONTAINS the word is not this machine.
            "https://localhost.attacker.example/rpc",
        ] {
            assert!(!is_local(url), "{url} was treated as local");
        }
    }

    /// The failure this module exists for: `ALL_PROXY=socks://…` (which is what
    /// `ureq` picks even when `HTTPS_PROXY` is also exported) asks for a LOCAL
    /// lookup, on a machine whose local lookups are the broken thing. Measured
    /// before the fix: 15.00s, `timeout: global`, on every registry call.
    #[test]
    fn a_socks5_proxy_is_made_to_resolve_at_the_far_end() {
        let from_env = Proxy::new("socks://127.0.0.1:10808").unwrap();
        assert!(
            from_env.resolve_target(),
            "ureq's socks5 default is the local lookup this test is about"
        );

        let fixed = resolve_at_the_proxy(&from_env).expect("socks5 must be rewritten");
        assert!(!fixed.resolve_target(), "the proxy must do the lookup");
        assert_eq!(fixed.host(), "127.0.0.1");
        assert_eq!(fixed.port(), 10808);
    }

    /// Credentials are part of reaching the proxy at all — a rebuild that drops
    /// them turns a working proxy into an authentication failure.
    #[test]
    fn credentials_survive_the_rebuild() {
        let from_env = Proxy::new("socks5://user:secret@127.0.0.1:1080").unwrap();
        let fixed = resolve_at_the_proxy(&from_env).expect("socks5 must be rewritten");
        assert_eq!(fixed.username(), Some("user"));
        assert_eq!(fixed.password(), Some("secret"));
        assert_eq!(fixed.host(), "127.0.0.1");
        assert_eq!(fixed.port(), 1080);
    }

    /// Everything that already resolves remotely is left exactly as `ureq`
    /// parsed it, `NO_PROXY` list included.
    #[test]
    fn proxies_that_already_resolve_remotely_are_untouched() {
        for spec in ["http://127.0.0.1:10808", "socks5h://127.0.0.1:1080"] {
            let proxy = Proxy::new(spec).unwrap();
            assert!(
                resolve_at_the_proxy(&proxy).is_none(),
                "{spec} needed no rewrite"
            );
        }
        // SOCKS4 cannot carry a hostname at all, so it is not asked to.
        let socks4 = Proxy::new("socks4://127.0.0.1:1080").unwrap();
        assert!(resolve_at_the_proxy(&socks4).is_none());
    }

    #[test]
    fn the_bypass_list_reads_as_entries() {
        // SAFETY: single-threaded test process; nothing else reads the
        // environment while this runs.
        unsafe { std::env::set_var("NO_PROXY", "localhost, 127.0.0.0/8 ,,::1") };
        let entries = no_proxy_from_env();
        unsafe { std::env::remove_var("NO_PROXY") };
        assert_eq!(entries, vec!["localhost", "127.0.0.0/8", "::1"]);
    }

    /// The 083 device answers a failed lookup with "不知道这样的主机。": a
    /// failed lookup is this machine's, whatever language Windows speaks.
    #[cfg(windows)]
    #[test]
    fn a_windows_lookup_failure_is_local_in_any_language() {
        for code in [11001, 11004] {
            let error = ureq::Error::Io(std::io::Error::from_raw_os_error(code));
            assert!(failed_inside_this_machine(&error), "{error:?}");
        }
        // WSAECONNREFUSED: something answered, so it left the machine.
        let refused = ureq::Error::Io(std::io::Error::from_raw_os_error(10061));
        assert!(!failed_inside_this_machine(&refused));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gvariant_strings_lose_their_quotes() {
        assert_eq!(unquote("'manual'"), "manual");
        // An integer arrives bare, and must not be mangled into "0808".
        assert_eq!(unquote("10808"), "10808");
        assert_eq!(unquote("''"), "");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_ignore_list_reads_as_entries() {
        assert_eq!(
            parse_gvariant_list("['localhost', '127.0.0.0/8', '::1']"),
            vec!["localhost", "127.0.0.0/8", "::1"]
        );
        // An empty list is no entries, not one empty entry — a `no_proxy("")`
        // would be a rule about nothing.
        assert!(parse_gvariant_list("@as []").is_empty());
        assert!(parse_gvariant_list("[]").is_empty());
    }
}

#[cfg(test)]
mod candidates {
    use super::*;

    /// The candidate list is process-global, and the two tests below install
    /// their own — serialised, or one test's chain answers the other's
    /// request.
    fn install(list: Vec<Candidate>) -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: Mutex<()> = Mutex::new(());
        let guard = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Ok(mut state) = state().lock() {
            *state = Some(Candidates::of(list));
        }
        guard
    }

    /// Leave no test-shaped state behind for the next test.
    fn uninstall() {
        if let Ok(mut guard) = state().lock() {
            *guard = None;
        }
    }

    /// A server on this machine that answers `ok` to `requests` requests.
    fn serve(requests: usize) -> String {
        let server = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback");
        let port = server.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in server.incoming().take(requests) {
                use std::io::{Read as _, Write as _};
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
            }
        });
        format!("http://127.0.0.1:{port}/")
    }

    /// A proxy that takes the connection and never says a word: what a
    /// black-holed host looks like from behind v2rayN. Kept alive by the
    /// caller; nothing accepts, the backlog completes the handshake.
    fn black_hole() -> (std::net::TcpListener, Proxy) {
        let hole = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback");
        let port = hole.local_addr().unwrap().port();
        let proxy = Proxy::new(&format!("http://127.0.0.1:{port}")).unwrap();
        (hole, proxy)
    }

    /// A proxy that takes the request and hangs up without a word: what
    /// v2rayN does when its node cannot reach the host (a proxy EOF).
    fn hangs_up(requests: usize) -> Proxy {
        let server = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback");
        let port = server.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in server.incoming().take(requests) {
                use std::io::Read as _;
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 1024];
                // Read the CONNECT first, so the close is a clean end of
                // stream rather than a reset over unread bytes.
                let _ = stream.read(&mut buf);
            }
        });
        Proxy::new(&format!("http://127.0.0.1:{port}")).unwrap()
    }

    fn generation() -> u64 {
        state().lock().unwrap().as_ref().unwrap().generation
    }

    /// One GET over `host`'s walk, and how many routes it took.
    fn get(host: &str, url: &str, timeout: Duration) -> (Result<String, Transport>, usize) {
        let mut tries = 0;
        let answer = walk(host, None, timeout, |agent| {
            tries += 1;
            agent.get(url).call()?.body_mut().read_to_string()
        });
        (answer, tries)
    }

    fn learned(host: &str) -> Option<usize> {
        let guard = state().lock().unwrap();
        guard.as_ref().and_then(|c| c.hosts.get(host).copied())
    }

    fn everyone() -> Option<usize> {
        state().lock().unwrap().as_ref().map(|c| c.current)
    }

    /// Spec 083 W13: a node black-holed behind the proxy timed out once, and
    /// the wallet's every other host went Direct for a minute. A timeout is
    /// about its host: the next host still starts at the proxy.
    #[test]
    fn a_timeout_moves_only_its_host() {
        let url = serve(2);
        let (_hole, proxy) = black_hole();
        let _serial = install(vec![Candidate::Env(proxy), Candidate::Direct]);
        let timeout = Duration::from_millis(300);

        let (answer, tries) = get("slow.example", &url, timeout);
        assert_eq!(answer.expect("direct after the proxy timed out"), "ok");
        assert_eq!(tries, 2);
        assert_eq!(everyone(), Some(0), "everyone else keeps the proxy");
        assert_eq!(learned("slow.example"), Some(1));

        let (answer, tries) = get("other.example", &url, timeout);
        assert_eq!(answer.expect("direct after the proxy timed out"), "ok");
        assert_eq!(tries, 2, "another host still started at the proxy");
        uninstall();
    }

    /// The route that carried a host is where its next request starts: one
    /// slow proxy attempt per host per derivation, not one per request.
    #[test]
    fn a_success_pins_the_route_that_worked() {
        let url = serve(2);
        let (_hole, proxy) = black_hole();
        let _serial = install(vec![Candidate::Env(proxy), Candidate::Direct]);
        let timeout = Duration::from_millis(300);

        let (answer, tries) = get("pinned.example", &url, timeout);
        assert_eq!(answer.expect("direct after the proxy timed out"), "ok");
        assert_eq!(tries, 2);

        let started = Instant::now();
        let (answer, tries) = get("pinned.example", &url, timeout);
        assert_eq!(answer.expect("direct, first"), "ok");
        assert_eq!(tries, 1, "the host went straight to the route that worked");
        assert!(started.elapsed() < timeout, "no proxy timeout paid again");
        assert_eq!(learned("pinned.example"), Some(1));
        uninstall();
    }

    /// Two requests refused by a dead proxy at the same moment. The shared
    /// index used to move twice — past Direct, off the end — and the second
    /// request failed without Direct ever being tried. Each request walks
    /// its own copy now, and a route moves everyone one step, not two.
    #[test]
    fn concurrent_failures_still_try_every_route() {
        let url = serve(2);
        let dead = Proxy::new("http://127.0.0.1:1").unwrap();
        let _serial = install(vec![Candidate::Env(dead), Candidate::Direct]);
        let together = std::sync::Arc::new(std::sync::Barrier::new(2));
        let requests: Vec<_> = ["a.example", "b.example"]
            .into_iter()
            .map(|host| {
                let (url, together) = (url.clone(), together.clone());
                std::thread::spawn(move || {
                    let mut tries = 0;
                    let answer = walk(host, None, Duration::from_secs(5), |agent| {
                        tries += 1;
                        if tries == 1 {
                            // Both on the proxy before either fails.
                            together.wait();
                        }
                        agent.get(&url).call()?.body_mut().read_to_string()
                    });
                    (answer.map_err(|failure| failure.error.to_string()), tries)
                })
            })
            .collect();
        for request in requests {
            let (answer, tries) = request.join().unwrap();
            assert_eq!(answer.expect("direct after the refusal"), "ok");
            assert_eq!(tries, 2, "the proxy, then Direct — none skipped");
        }
        assert_eq!(everyone(), Some(1), "a refusing proxy moves everyone, once");
        uninstall();
    }

    /// When every route failed, the caller hears the failure that says most
    /// about the far end — not simply the last route's.
    #[test]
    fn the_most_telling_failure_is_reported() {
        let through = Candidate::Env(Proxy::new("http://127.0.0.1:10808").unwrap());
        let proxy = |error| Failure {
            route_is_dead: route_is_dead(&through, None, &error),
            direct: false,
            error,
        };
        let direct = |error| Failure {
            route_is_dead: false,
            direct: true,
            error,
        };
        let refused = || ureq::Error::Io(std::io::ErrorKind::ConnectionRefused.into());
        let pick = |first, then| {
            let mut heard = Heard::default();
            heard.failed(first);
            heard.failed(then);
            heard.into_transport()
        };

        // The sponsorship case: timed out through the proxy — it may have
        // been paid — then Direct could not look the name up. The relay
        // reads `error` (`pending_unknown`); `local` is still the direct
        // attempt's, as before 083.
        let heard = pick(
            proxy(ureq::Error::Timeout(ureq::Timeout::Global)),
            direct(ureq::Error::HostNotFound),
        );
        assert!(matches!(heard.error, ureq::Error::Timeout(_)));
        assert!(heard.local, "Direct could not even look the name up");

        // The offline laptop with v2rayN up: xray takes the CONNECT and its
        // outbound fails, Direct finds no such host. The proxy's words are
        // reported, but it is still "your network" — the login probe must
        // say so, not that the Passkey Index is down (spec 038).
        for through_the_proxy in [
            ureq::Error::ConnectProxyFailed("proxy server did not respond".to_owned()),
            ureq::Error::Io(std::io::ErrorKind::UnexpectedEof.into()),
        ] {
            let heard = pick(proxy(through_the_proxy), direct(ureq::Error::HostNotFound));
            assert!(!matches!(heard.error, ureq::Error::HostNotFound));
            assert!(heard.local, "offline is local: {:?}", heard.error);
        }

        // A route existed and the far end did not answer.
        let heard = pick(
            proxy(ureq::Error::Timeout(ureq::Timeout::Global)),
            direct(ureq::Error::Io(std::io::ErrorKind::ConnectionReset.into())),
        );
        assert!(!heard.local);

        // Nothing left the machine: the direct attempt speaks, and says so.
        let heard = pick(proxy(refused()), direct(ureq::Error::HostNotFound));
        assert!(matches!(heard.error, ureq::Error::HostNotFound));
        assert!(heard.local);

        // Two answers from beyond the machine: the intended route's stands.
        let heard = pick(
            proxy(ureq::Error::ConnectProxyFailed("502".to_owned())),
            direct(ureq::Error::Io(std::io::ErrorKind::ConnectionReset.into())),
        );
        assert!(matches!(heard.error, ureq::Error::ConnectProxyFailed(_)));

        // ...unless a later route timed out: that request may have arrived.
        let heard = pick(
            proxy(ureq::Error::ConnectProxyFailed("502".to_owned())),
            direct(ureq::Error::Timeout(ureq::Timeout::Global)),
        );
        assert!(matches!(heard.error, ureq::Error::Timeout(_)));
    }

    /// The list is re-derived on its schedule, and what hosts learned goes
    /// with it; a request that started on the old list writes nothing into
    /// the new one. At a dead end the list is dropped only when everyone's
    /// route had moved — a single host's dead end forgets that host alone.
    #[test]
    fn the_list_is_re_derived_and_learning_starts_over() {
        let dead = Proxy::new("http://127.0.0.1:1").unwrap();
        let _serial = install(vec![Candidate::Env(dead), Candidate::Direct]);
        let old = state().lock().unwrap().as_ref().unwrap().generation;
        step_past(old, "slow.example", 0, false);
        assert_eq!(learned("slow.example"), Some(1));
        assert_eq!(everyone(), Some(0));

        // A host's own dead end forgets that host; the list stands.
        exhausted(old, "slow.example");
        assert_eq!(learned("slow.example"), None);
        assert!(state().lock().unwrap().is_some());

        // Everyone's route moved, then a walk ran out: the list goes.
        step_past(old, "slow.example", 0, true);
        assert_eq!(everyone(), Some(1));
        exhausted(old, "slow.example");
        assert!(state().lock().unwrap().is_none());

        // On its schedule: a minute-old list is derived afresh.
        let mut minute_old = Candidates::of(vec![Candidate::Direct]);
        minute_old.derived_at = Instant::now()
            .checked_sub(REDERIVE_AFTER)
            .expect("a machine up longer than a minute");
        minute_old.hosts.insert("pinned.example".to_owned(), 0);
        let stale = minute_old.generation;
        *state().lock().unwrap() = Some(minute_old);
        let _ = current();
        let (generation, forgot, current, ends_direct) = {
            let guard = state().lock().unwrap();
            let fresh = guard.as_ref().unwrap();
            (
                fresh.generation,
                fresh.hosts.is_empty(),
                fresh.current,
                matches!(fresh.list.last(), Some(Candidate::Direct)),
            )
        };
        assert_ne!(generation, stale, "derived afresh");
        assert!(forgot, "nothing learned survives");
        assert_eq!(current, 0);
        assert!(ends_direct, "Direct is always last");

        // What the old list's requests learn now lands nowhere.
        learn(stale, |candidates| {
            candidates.hosts.insert("slow.example".to_owned(), 1);
        });
        step_past(stale, "slow.example", 0, true);
        assert_eq!(learned("slow.example"), None);
        assert_eq!(everyone(), Some(0));
        exhausted(stale, "slow.example");
        assert!(
            state().lock().unwrap().is_some(),
            "not dropped by an old walk"
        );
        uninstall();
    }

    /// Only a failure every host would meet moves everyone: the proxy
    /// refused or could not be reached. The rule chosen over the design
    /// doc's is locked here — a reset, a TLS failure, a proxy's `502` or its
    /// hang-up, a timeout are each about one host (083 W13 review).
    #[test]
    fn only_a_proxy_that_cannot_be_reached_is_dead() {
        let http = Candidate::Env(Proxy::new("http://127.0.0.1:10808").unwrap());
        let io = |kind: std::io::ErrorKind| ureq::Error::Io(kind.into());
        for dead in [
            io(std::io::ErrorKind::ConnectionRefused),
            io(std::io::ErrorKind::NetworkUnreachable),
            io(std::io::ErrorKind::HostUnreachable),
            // The proxy's own name did not resolve: an HTTP proxy resolves
            // the target itself, so the only name looked up here is its own.
            ureq::Error::HostNotFound,
        ] {
            assert!(route_is_dead(&http, None, &dead), "{dead:?} is the proxy");
        }
        for about_the_host in [
            io(std::io::ErrorKind::ConnectionReset),
            io(std::io::ErrorKind::UnexpectedEof),
            ureq::Error::ConnectProxyFailed("502 Bad Gateway".to_owned()),
            ureq::Error::ConnectProxyFailed("proxy server did not respond".to_owned()),
            ureq::Error::Tls("certificate expired"),
            ureq::Error::Timeout(ureq::Timeout::Global),
        ] {
            assert!(
                !route_is_dead(&http, None, &about_the_host),
                "{about_the_host:?} may be one host's"
            );
        }
        // SOCKS4 has the TARGET looked up here: a name that will not resolve
        // is the site's, not the proxy's.
        let socks4 = Candidate::Env(Proxy::new("socks4://127.0.0.1:1080").unwrap());
        assert!(!route_is_dead(&socks4, None, &ureq::Error::HostNotFound));
        assert!(route_is_dead(
            &socks4,
            None,
            &io(std::io::ErrorKind::ConnectionRefused)
        ));
        // Direct has no proxy to die.
        assert!(!route_is_dead(
            &Candidate::Direct,
            None,
            &io(std::io::ErrorKind::ConnectionRefused)
        ));
    }

    /// A proxy that takes the request and hangs up — v2rayN whose node
    /// cannot reach this host — moves that host alone, like a timeout.
    #[test]
    fn a_proxy_that_hangs_up_moves_only_its_host() {
        let url = serve(1);
        let _serial = install(vec![Candidate::Env(hangs_up(1)), Candidate::Direct]);
        let (answer, tries) = get("eof.example", &url, Duration::from_secs(5));
        assert_eq!(answer.expect("direct after the proxy hung up"), "ok");
        assert_eq!(tries, 2);
        assert_eq!(everyone(), Some(0), "everyone else keeps the proxy");
        assert_eq!(learned("eof.example"), Some(1));
        uninstall();
    }

    /// The founder's Mac from a terminal: the system proxy works, the dead
    /// `all_proxy` behind it refuses. One slow host walking through both
    /// used to push everyone past the working proxy onto Direct — from
    /// behind a firewall, W13 again (083 W13 review).
    #[test]
    fn a_refusal_further_down_never_skips_a_route_that_works() {
        let url = serve(1);
        // Fails this one host (its node is unreachable) and no other.
        let working_for_others = hangs_up(1);
        let refusing = Proxy::new("http://127.0.0.1:1").unwrap();
        let _serial = install(vec![
            Candidate::System(working_for_others),
            Candidate::Env(refusing),
            Candidate::Direct,
        ]);
        // Seconds: Windows takes a while to report a refusal on loopback.
        let (answer, tries) = get("slow.example", &url, Duration::from_secs(5));
        assert_eq!(answer.expect("direct at last"), "ok");
        assert_eq!(tries, 3);
        assert_eq!(
            everyone(),
            Some(0),
            "the system proxy still carries everyone"
        );

        // A host moved off the first proxy skips the dead one: it fails
        // every host.
        step_past(generation(), "other.example", 0, false);
        assert_eq!(begin("other.example").start, 2);
        assert_eq!(begin("new.example").start, 0);
        uninstall();
    }

    /// 083 W13 review: on Windows the proxy's bypass list is read with it
    /// (`ProxyOverride`; v2rayN's covers `127.*` and `192.168.*`), and
    /// `ureq` connects to such a target directly. A LAN node that is down
    /// refuses — and that refusal is the node's, not the proxy's: the
    /// wallet must not go Direct on every poll.
    #[test]
    fn a_bypassed_host_that_refuses_is_not_the_proxy_dying() {
        // Pointed at a black hole: were the target NOT bypassed, the
        // request would sit there, not be refused.
        let (_hole, via) = black_hole();
        let bypassing = Proxy::builder(ProxyProtocol::Http)
            .host(via.host())
            .port(via.port())
            .no_proxy("127.0.0.1")
            .build()
            .unwrap();
        let _serial = install(vec![Candidate::Env(bypassing.clone()), Candidate::Direct]);
        let url = "http://127.0.0.1:1/";
        let target: Uri = url.parse().unwrap();
        let failure = walk(
            "127.0.0.1",
            Some(&target),
            Duration::from_secs(5),
            |agent| agent.get(url).call().map(|_| ()),
        )
        .expect_err("nothing listens there");
        assert!(
            matches!(&failure.error, ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused),
            "the node itself refused: {:?}",
            failure.error
        );
        assert_eq!(everyone(), Some(0), "the node refused, not the proxy");
        assert!(state().lock().unwrap().is_some(), "and the list stands");

        // The same refusal with no target to judge reads as the proxy's.
        let refused = ureq::Error::Io(std::io::ErrorKind::ConnectionRefused.into());
        let candidate = Candidate::Env(bypassing);
        assert!(route_is_dead(&candidate, None, &refused));
        assert!(!route_is_dead(&candidate, Some(&target), &refused));
        let elsewhere: Uri = "https://rpc.example/".parse().unwrap();
        assert!(route_is_dead(&candidate, Some(&elsewhere), &refused));
        uninstall();
    }

    /// A proxy that accepts and reaches nothing — its upstream node is dead
    /// — never refuses, so the refusal rule alone would leave [`agent`]'s
    /// callers and the caBLE tunnel on it for good. Three hosts that had to
    /// walk past it to Direct move everyone; one or two do not.
    #[test]
    fn a_proxy_three_hosts_walk_past_moves_everyone() {
        let url = serve(WALKED_PAST_BY);
        let (_hole, proxy) = black_hole();
        let _serial = install(vec![Candidate::Env(proxy), Candidate::Direct]);
        let timeout = Duration::from_millis(300);
        for (walked, host) in ["one.example", "two.example", "three.example"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(everyone(), Some(0), "{walked} walked past: not yet");
            let (answer, tries) = get(host, &url, timeout);
            assert_eq!(answer.expect("direct after the proxy timed out"), "ok");
            assert_eq!(tries, 2);
        }
        assert_eq!(everyone(), Some(1), "a proxy that reaches nothing");
        assert!(current().proxy().is_none(), "agent() goes Direct");
        uninstall();
    }

    /// The count is of hosts with no success through the proxy between: a
    /// proxy that carries anyone is not dead. And a host whose own route is
    /// the proxy keeps it when everyone else moves on — other hosts'
    /// failures never reroute it (083 D3). A host that failed everywhere (a
    /// dead dApp) is no evidence against the proxy at all.
    #[test]
    fn a_success_through_the_proxy_restarts_the_count() {
        let (_hole, proxy) = black_hole();
        let _serial = install(vec![Candidate::Env(proxy), Candidate::Direct]);
        let generation = generation();
        carried(generation, "pinned.example", 0, true, &[]);
        carried(generation, "a.example", 1, false, &[0]);
        carried(generation, "b.example", 1, false, &[0]);
        assert_eq!(everyone(), Some(0));
        carried(generation, "pinned.example", 0, true, &[]);
        carried(generation, "c.example", 1, false, &[0]);
        carried(generation, "d.example", 1, false, &[0]);
        assert_eq!(everyone(), Some(0), "the count restarted at the success");

        // A dead dApp: failed through the proxy, then Direct too.
        step_past(generation, "dead.example", 0, false);
        exhausted(generation, "dead.example");
        assert_eq!(everyone(), Some(0), "a site down everywhere says nothing");

        carried(generation, "e.example", 1, false, &[0]);
        assert_eq!(everyone(), Some(1));
        assert_eq!(
            begin("pinned.example").start,
            0,
            "its own route, whatever others met"
        );
        assert_eq!(begin("new.example").start, 1);
        uninstall();
    }

    /// Every walking caller names its url, and what is learned is keyed on
    /// the host: `RPC.Example:443/a` and `rpc.example/b` are one entry, and
    /// a loopback url runs direct, once, and learns nothing.
    #[test]
    fn a_url_is_learned_under_its_host() {
        let url = serve(2);
        let (_hole, proxy) = black_hole();
        let _serial = install(vec![Candidate::Env(proxy), Candidate::Direct]);
        let timeout = Duration::from_millis(300);
        let fetch = |named: &str| {
            let mut tries = 0;
            let answer = with_candidates_for(named, timeout, |agent| {
                tries += 1;
                agent.get(&url).call()?.body_mut().read_to_string()
            });
            (answer, tries)
        };

        let (answer, tries) = fetch("https://RPC.Example:443/a");
        assert_eq!(answer.expect("direct after the proxy timed out"), "ok");
        assert_eq!(tries, 2);
        assert_eq!(learned("rpc.example"), Some(1));

        let (answer, tries) = fetch("https://rpc.example/b");
        assert_eq!(answer.expect("direct, first"), "ok");
        assert_eq!(tries, 1, "one host, one entry");

        let mut tries = 0;
        let local = with_candidates_for("http://127.0.0.1:8545/", timeout, |_agent| {
            tries += 1;
            Ok::<_, ureq::Error>(())
        });
        assert!(local.is_ok());
        assert_eq!(tries, 1);
        let hosts = state().lock().unwrap().as_ref().map(|c| c.hosts.len());
        assert_eq!(hosts, Some(1), "loopback learned nothing");
        uninstall();
    }

    /// The dictionary captured on the founder's Mac on 2026-09-11 — the one
    /// whose SOCKS entry named a port nothing listened on.
    const CAPTURED: &str = "<dictionary> {
  ExcludeSimpleHostnames : 1
  HTTPEnable : 1
  HTTPPort : 1088
  HTTPProxy : 127.0.0.1
  HTTPSEnable : 1
  HTTPSPort : 1088
  HTTPSProxy : 127.0.0.1
  SOCKSEnable : 1
  SOCKSPort : 1080
  SOCKSProxy : 127.0.0.1
  ExceptionsList : <array> {
    0 : *.local
    1 : 169.254/16
  }
}";

    #[test]
    fn scutil_prefers_https_and_carries_the_exceptions() {
        let proxy = proxy_from_scutil(CAPTURED).expect("a proxy");
        assert_eq!(proxy.protocol(), ProxyProtocol::Http);
        assert_eq!(proxy.host(), "127.0.0.1");
        assert_eq!(proxy.port(), 1088, "HTTPS wins over the dead SOCKS entry");
        let local: ureq::http::Uri = "http://something.local/".parse().unwrap();
        assert!(proxy.is_no_proxy(&local));
    }

    #[test]
    fn scutil_skips_disabled_and_half_configured_entries() {
        let socks_only = "<dictionary> {
  HTTPSEnable : 0
  HTTPSPort : 1088
  HTTPSProxy : 127.0.0.1
  HTTPEnable : 1
  HTTPPort : 0
  HTTPProxy : 127.0.0.1
  SOCKSEnable : 1
  SOCKSPort : 1080
  SOCKSProxy : 127.0.0.1
}";
        let proxy = proxy_from_scutil(socks_only).expect("the SOCKS entry");
        assert_eq!(
            proxy.protocol(),
            ProxyProtocol::Socks5h,
            "resolved at the far end"
        );
        assert_eq!(proxy.port(), 1080);
        assert!(proxy_from_scutil("<dictionary> {\n  HTTPEnable : 0\n}").is_none());
    }

    #[test]
    fn a_route_named_by_both_sources_is_one_candidate() {
        let a = Candidate::System(Proxy::new("http://127.0.0.1:1088").unwrap());
        let b = Candidate::Env(Proxy::new("http://127.0.0.1:1088").unwrap());
        let c = Candidate::Env(Proxy::new("socks5h://127.0.0.1:1080").unwrap());
        assert!(a.same_route(&b));
        assert!(!a.same_route(&c));
        assert!(!a.same_route(&Candidate::Direct));
    }

    /// The failure that started spec 038 Part B: a proxy in the environment
    /// that refuses every connection, on a machine that can reach the host
    /// directly. The chain must end on a success, not on a sentence about
    /// our service.
    #[test]
    fn a_refusing_proxy_falls_through_to_direct() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback");
        let port = server.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in server.incoming().take(1) {
                use std::io::{Read as _, Write as _};
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
            }
        });
        // A proxy nobody listens on, then direct.
        let dead = Proxy::new("http://127.0.0.1:1").unwrap();
        let _serial = install(vec![Candidate::Env(dead), Candidate::Direct]);
        let url = format!("http://127.0.0.1:{port}/");
        // `walk`, not `with_candidates_for`: a loopback url never takes a
        // proxy there, and this server is on loopback.
        let body = walk(
            "fallthrough.example",
            None,
            Duration::from_secs(5),
            |agent| agent.get(&url).call()?.body_mut().read_to_string(),
        )
        .expect("direct must succeed after the proxy refused");
        assert_eq!(body, "ok");
        // Leave no test-shaped state behind for the next test.
        if let Ok(mut guard) = state().lock() {
            *guard = None;
        }
    }

    #[test]
    fn exhausting_every_candidate_names_where_it_failed() {
        let dead = Proxy::new("http://127.0.0.1:1").unwrap();
        let _serial = install(vec![Candidate::Env(dead), Candidate::Direct]);
        // A name no resolver answers: the direct attempt fails INSIDE the
        // machine, so the verdict is local.
        // The budget is a ceiling, not the subject: under a sing-box TUN
        // (the 083 Windows device) "no such host" takes 11 s to arrive, and
        // at 5 s the attempt timed out before the resolver said no.
        let url = "http://vela-038-no-such-host.invalid/";
        let failure = with_candidates_for(url, Duration::from_secs(30), |agent| {
            agent.get(url).call().map(|_| ())
        })
        .expect_err("nothing can answer");
        assert!(
            failure.local,
            "unresolvable host = this machine could not get out: {:?}",
            failure.error
        );
        // The dead-end list was dropped, so the next request re-derives.
        assert!(state().lock().map(|g| g.is_none()).unwrap_or(false));
    }
}
