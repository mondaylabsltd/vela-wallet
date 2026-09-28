//! The app's one HTTP agent, and how each request gets out of the machine.
//!
//! **Everything that talks to the network goes through this module.** Not by
//! convention — there is exactly one place an agent is constructed, and it is
//! this one, so a caller cannot get a client that skipped the proxy by
//! forgetting a line.
//!
//! ## The rule (spec 082 ruling 3, RD2, RD9)
//!
//! The wallet's own traffic follows the system proxy — PAC included — the way
//! WebKit does, **per request**, and never falls back to a direct connection
//! the system did not name:
//!
//! 1. [`routes_for`] asks the platform for the ordered routes of ONE URL. On
//!    macOS that is CFNetwork's own resolver (`proxy_macos.rs`): the exception
//!    list, simple hostnames, per-scheme proxies and PAC, the answer WebKit
//!    gets. DIRECT is a route only when that list says DIRECT.
//! 2. [`with_routes`] walks them, and moves to the next route **only when the
//!    TCP connect to the proxy itself failed** (refused, unreachable, its name
//!    did not resolve, the connect timed out). A timeout after connecting, a
//!    TLS failure, a CONNECT answered 5xx or closed with no reply all mean the
//!    proxy answered for that host — the request stops there, with that
//!    error.
//! 3. Nothing survives between two requests: no "current route", no global
//!    switch to direct after one timeout (W8's doubled timeouts), so a proxy
//!    that comes back is used by the very next request.
//!
//! A failure that is the proxy's own carries a [`ProxyFailure`] and is
//! logged — `proxy: 127.0.0.1:9 unreachable`, `proxy: PAC … failed` — because
//! "when the proxy fails, say so" (ruling 3).
//!
//! ## Per platform
//!
//! * **macOS** — `proxy_macos.rs`. No environment proxies: a GUI app's
//!   environment is whatever launched it, and a dead `all_proxy` exported in a
//!   developer's shell is how "the index is unreachable" appeared under a
//!   healthy index (spec 038 finding 11).
//! * **Linux** — GNOME's `org.gnome.system.proxy` when it is manual, else the
//!   environment. PAC (`mode = 'auto'`) is not supported; that is logged once
//!   and the request goes direct, as before this module knew about PAC.
//! * **Windows** — `ureq`'s `win-system-proxy` reads the WinINET registry keys
//!   inside `Proxy::try_from_env()`; the environment otherwise. PAC is not
//!   supported there either, and is logged once.
//!
//! Loopback is always direct (a local node behind a system proxy is not
//! "outside"). A SOCKS5 proxy from the environment resolves at the far end
//! (`socks5h`): on the machines that need a proxy, the local lookup is the
//! broken step.
//!
//! ## A dev build pointed at a fault proxy
//!
//! Spec 082's bad-network rows aim a fault proxy at THIS app and nothing else
//! on the machine. [`dev_proxy`] (`VELA_DEV_PROXY=<host>:<port>`,
//! `dev-fixtures` builds only) is then the one route of every request,
//! loopback included — a fault the proxy injects must reach the app, not be
//! routed around. The in-app browser takes the same proxy (`webview.rs`).

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use ureq::{Agent, Proxy, ProxyProtocol};

use crate::diag::vlog;

/// How long a connection may take to be set up — TCP, a proxy's CONNECT, TLS —
/// within a request's own budget. Shorter than that budget on purpose: ureq
/// names a timeout by the phase that ran out, and only a connect-phase
/// timeout proves the request was never written (spec 082 RA1), so the phase
/// must be able to run out first.
const CONNECT_BUDGET: Duration = Duration::from_secs(5);

/// One way out of the machine for one request.
#[derive(Clone, Debug)]
pub enum Route {
    /// No proxy — only where the system (or loopback) says so.
    Direct,
    /// An HTTP proxy, reached with CONNECT; the target resolves at the proxy.
    HttpConnect { host: String, port: u16 },
    /// A SOCKS5 proxy that resolves the target at the far end.
    Socks5h { host: String, port: u16 },
    /// A proxy from the environment (Linux, Windows), carried whole — its
    /// credentials and bypass list included.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Env(Proxy),
}

impl Route {
    /// `host:port` of the proxy, or `direct` — what a log line names.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Direct => "direct".to_owned(),
            Self::HttpConnect { host, port } | Self::Socks5h { host, port } => {
                format!("{host}:{port}")
            }
            Self::Env(proxy) => format!("{}:{}", proxy.host(), proxy.port()),
        }
    }

    fn proxy(&self) -> Option<Proxy> {
        let build = |protocol, host: &str, port: u16| {
            Proxy::builder(protocol)
                .host(host)
                .port(port)
                .resolve_target(false)
                .build()
                .ok()
        };
        match self {
            Self::Direct => None,
            Self::HttpConnect { host, port } => build(ProxyProtocol::Http, host, *port),
            Self::Socks5h { host, port } => build(ProxyProtocol::Socks5h, host, *port),
            Self::Env(proxy) => Some(proxy.clone()),
        }
    }
}

/// How a proxy failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyFailureKind {
    /// The TCP connect to the proxy was refused, or its name or its network
    /// could not be reached.
    Unreachable,
    /// The TCP connect to the proxy timed out.
    Timeout,
    /// The PAC file could not be fetched or run.
    PacFailed,
    /// The proxy answered and would not open the tunnel (a 5xx, or a close
    /// with no reply). It spoke for the host, so this is the SITE's failure
    /// class, never "your proxy isn't responding" (research RX).
    RefusedTunnel,
}

impl ProxyFailureKind {
    fn words(self) -> &'static str {
        match self {
            Self::Unreachable => "unreachable",
            Self::Timeout => "timed out",
            Self::PacFailed => "PAC failed",
            Self::RefusedTunnel => "refused tunnel",
        }
    }

    /// The proxy itself could not be used — the browser's `proxy` class
    /// (`probe_code::PROXY`, spec 082 RD9).
    #[must_use]
    pub fn is_the_proxys(self) -> bool {
        !matches!(self, Self::RefusedTunnel)
    }
}

/// A failure that was the proxy's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyFailure {
    /// `host:port`, or the PAC file's host.
    pub proxy: String,
    pub kind: ProxyFailureKind,
}

/// A request that did not get an answer from the server.
#[derive(Debug)]
pub struct Transport {
    /// The last route's error, verbatim. For a PAC that could not run, no
    /// route was tried and this is `ConnectionFailed`.
    pub error: ureq::Error,
    /// It never got out of this machine: the proxy (or PAC) could not be
    /// used, or a direct attempt could not resolve or route to the host.
    /// `false` means a route existed and the far end did not answer.
    pub local: bool,
    /// The proxy's own failure, when it was one.
    pub proxy: Option<ProxyFailure>,
}

/// Is this url on this machine?
fn is_local(url: &str) -> bool {
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
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(std::net::IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

/// The routes one request to `url` may take, in order — or the proxy failure
/// that leaves it none (a PAC that could not run: never a silent DIRECT).
pub fn routes_for(url: &str) -> Result<Vec<Route>, ProxyFailure> {
    if let Some(dev) = dev_proxy() {
        return Ok(vec![Route::HttpConnect {
            host: dev.host().to_owned(),
            port: dev.port(),
        }]);
    }
    if is_local(url) {
        return Ok(vec![Route::Direct]);
    }
    platform_routes(url)
}

#[cfg(target_os = "macos")]
fn platform_routes(url: &str) -> Result<Vec<Route>, ProxyFailure> {
    use crate::executor::proxy_macos::{self, Entry};
    match proxy_macos::routes_for(url) {
        Ok(entries) => {
            let routes: Vec<Route> = entries
                .into_iter()
                .map(|entry| match entry {
                    Entry::Direct => Route::Direct,
                    Entry::Http { host, port } => Route::HttpConnect { host, port },
                    Entry::Socks { host, port } => Route::Socks5h { host, port },
                })
                .collect();
            // An empty list names no proxy at all.
            Ok(if routes.is_empty() {
                vec![Route::Direct]
            } else {
                routes
            })
        }
        Err(failed) => {
            vlog!(
                "proxy",
                "PAC {} failed: {} (no direct fall-back)",
                failed.pac,
                failed.reason
            );
            Err(ProxyFailure {
                proxy: failed.pac,
                kind: ProxyFailureKind::PacFailed,
            })
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn platform_routes(_url: &str) -> Result<Vec<Route>, ProxyFailure> {
    static PAC_NOTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if pac_configured() && !PAC_NOTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        vlog!(
            "proxy",
            "the system names a PAC file, which this platform cannot run; going direct"
        );
    }
    if let Some(system) = desktop_proxy() {
        return Ok(vec![Route::Env(system)]);
    }
    if let Some(from_env) = Proxy::try_from_env() {
        // On Windows this also reads the registry (`win-system-proxy`).
        return Ok(vec![Route::Env(
            resolve_at_the_proxy(&from_env).unwrap_or(from_env),
        )]);
    }
    Ok(vec![Route::Direct])
}

#[cfg(target_os = "linux")]
fn pac_configured() -> bool {
    gsettings("org.gnome.system.proxy", "mode").as_deref() == Some("auto")
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn pac_configured() -> bool {
    false
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn desktop_proxy() -> Option<Proxy> {
    // Windows: `ureq`'s `win-system-proxy` feature reads WinINET inside
    // `Proxy::try_from_env()`, so the system setting arrives there.
    None
}

/// `VELA_DEV_PROXY=<host>:<port>`: the ONE route out of a dev build (module
/// note). Two gates, like `parallel_space::active`: without `dev-fixtures`
/// this is a constant `None` and the variable is never read. Read once; a
/// value that is set but not `<host>:<port>` stops the app rather than
/// letting a fault row run over the ordinary route.
pub fn dev_proxy() -> Option<&'static Proxy> {
    #[cfg(feature = "dev-fixtures")]
    {
        static DEV: OnceLock<Option<Proxy>> = OnceLock::new();
        DEV.get_or_init(|| {
            let value = std::env::var("VELA_DEV_PROXY").ok()?;
            let Some(proxy) = parse_dev_proxy(&value) else {
                eprintln!("[vela-wallet] dev proxy: {value:?} is not <host>:<port>");
                std::process::exit(2);
            };
            eprintln!(
                "[vela-wallet] dev proxy: all traffic via {}:{}",
                proxy.host(),
                proxy.port()
            );
            Some(proxy)
        })
        .as_ref()
    }
    #[cfg(not(feature = "dev-fixtures"))]
    {
        None
    }
}

/// `127.0.0.1:8899` → an HTTP CONNECT proxy that resolves at the far end and
/// bypasses nothing (no `NO_PROXY`: the builder does not read it).
#[cfg(feature = "dev-fixtures")]
fn parse_dev_proxy(value: &str) -> Option<Proxy> {
    let (host, port) = value.trim().rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|&port| port != 0)?;
    if host.is_empty() || host.contains(['/', '@', ':']) {
        return None;
    }
    Proxy::builder(ProxyProtocol::Http)
        .host(host)
        .port(port)
        .build()
        .ok()
}

/// The first route of a request to `url` — for the one caller that dials a
/// socket of its own (the caBLE tunnel) and so cannot walk the list.
pub fn first_route(url: &str) -> Result<Route, ProxyFailure> {
    routes_for(url).map(|routes| routes.into_iter().next().unwrap_or(Route::Direct))
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
            ) || resolver_said_no(&io.to_string())
        }
        _ => false,
    }
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

/// Did the TCP connect to `route`'s PROXY fail — the one failure that moves a
/// request to the next route? `None` for a direct route, and for every
/// failure after the proxy took the connection.
///
/// A connect-phase timeout is ambiguous — ureq's connect phase covers the
/// TCP connect, the CONNECT exchange and TLS — so it counts as the proxy's
/// only when a fresh TCP connect to the proxy fails too.
fn proxy_connect_failed(route: &Route, error: &ureq::Error) -> Option<ProxyFailureKind> {
    use std::io::ErrorKind;
    if matches!(route, Route::Direct) {
        return None;
    }
    match error {
        ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Timeout(ureq::Timeout::Resolve) => Some(ProxyFailureKind::Unreachable),
        ureq::Error::Io(io)
            if matches!(
                io.kind(),
                ErrorKind::ConnectionRefused
                    | ErrorKind::AddrNotAvailable
                    | ErrorKind::HostUnreachable
                    | ErrorKind::NetworkUnreachable
                    | ErrorKind::NetworkDown
            ) || resolver_said_no(&io.to_string()) =>
        {
            Some(ProxyFailureKind::Unreachable)
        }
        ureq::Error::Timeout(ureq::Timeout::Connect) if !proxy_answers(route) => {
            Some(ProxyFailureKind::Timeout)
        }
        _ => None,
    }
}

/// Does the proxy take a TCP connection right now? One short attempt.
fn proxy_answers(route: &Route) -> bool {
    use std::net::ToSocketAddrs as _;
    let Some(proxy) = route.proxy() else {
        return true;
    };
    let Ok(addrs) = (proxy.host(), proxy.port()).to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_ok())
}

/// Walk `routes` for one request: the first answer, or the first failure
/// that is not the proxy's own TCP connect — which moves on to the next
/// route. `attempt` performs the request over one route.
fn walk<T>(
    routes: &[Route],
    mut attempt: impl FnMut(&Route) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    let mut last: Option<(ureq::Error, ProxyFailure)> = None;
    for route in routes {
        match attempt(route) {
            Ok(value) => return Ok(value),
            Err(error) => {
                if let Some(kind) = proxy_connect_failed(route, &error) {
                    let failure = ProxyFailure {
                        proxy: route.label(),
                        kind,
                    };
                    vlog!("proxy", "{} {}", failure.proxy, kind.words());
                    last = Some((error, failure));
                    continue;
                }
                let refused = matches!(error, ureq::Error::ConnectProxyFailed(_)).then(|| {
                    let failure = ProxyFailure {
                        proxy: route.label(),
                        kind: ProxyFailureKind::RefusedTunnel,
                    };
                    vlog!("proxy", "{} {}", failure.proxy, failure.kind.words());
                    failure
                });
                return Err(Transport {
                    local: matches!(route, Route::Direct) && failed_inside_this_machine(&error),
                    error,
                    proxy: refused,
                });
            }
        }
    }
    Err(match last {
        Some((error, failure)) => Transport {
            error,
            local: true,
            proxy: Some(failure),
        },
        None => Transport {
            error: ureq::Error::ConnectionFailed,
            local: true,
            proxy: None,
        },
    })
}

/// Run one request to `url` over its routes (module note). `call` is
/// invoked with an agent for each route it tries; the first `Ok` — or the
/// first error that is not the proxy's own TCP connect, which is the far end
/// (or the proxy speaking for it) — is returned as-is.
pub fn with_routes<T>(
    url: &str,
    timeout: Duration,
    call: impl FnMut(&Agent) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    over_routes(routes_for(url), timeout, call)
}

/// [`with_routes`] over routes already resolved — the seam the tests use.
fn over_routes<T>(
    routes: Result<Vec<Route>, ProxyFailure>,
    timeout: Duration,
    mut call: impl FnMut(&Agent) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    let routes = match routes {
        Ok(routes) => routes,
        // No route at all (a PAC that could not run): nothing is tried, and
        // above all not a direct connection the system did not name.
        Err(failure) => {
            return Err(Transport {
                error: ureq::Error::ConnectionFailed,
                local: true,
                proxy: Some(failure),
            });
        }
    };
    walk(&routes, |route| call(&agent_over(route, timeout)))
}

/// One agent per route and timeout, kept for the life of the process.
///
/// A `ureq` agent IS its connection pool. Building a fresh one for every
/// request reused nothing: each RPC call, index fetch and rate read paid DNS,
/// TCP and a TLS handshake of its own (the 078 loading audit). Agents are
/// cheap handles over a shared pool, so the cache hands out clones. This is a
/// cache of connections, not a memory of which route worked.
fn agent_over(route: &Route, timeout: Duration) -> Agent {
    static AGENTS: OnceLock<Mutex<std::collections::HashMap<String, Agent>>> = OnceLock::new();
    let key = format!("{route:?}|{}", timeout.as_millis());
    let agents = AGENTS.get_or_init(Mutex::default);
    if let Ok(agents) = agents.lock()
        && let Some(agent) = agents.get(&key)
    {
        return agent.clone();
    }
    let config = Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some((timeout * 4 / 5).min(CONNECT_BUDGET)))
        // A balance read talks to two dozen chains' endpoints at once; the
        // default ten idle sockets would drop most of them between calls.
        .max_idle_connections(128)
        .max_idle_connections_per_host(8)
        // `None` is "no proxy" — never ureq's own pick from the environment.
        .proxy(route.proxy());
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
#[cfg_attr(all(target_os = "macos", not(test)), allow(dead_code))]
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
#[cfg_attr(all(target_os = "macos", not(test)), allow(dead_code))]
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

    /// `VELA_DEV_PROXY` is `<host>:<port>` and nothing else, and what it names
    /// is a CONNECT proxy that bypasses nothing — loopback and a developer
    /// shell's `NO_PROXY` included — or a fault row could be routed around.
    #[cfg(feature = "dev-fixtures")]
    #[test]
    fn the_dev_proxy_is_host_and_port_and_bypasses_nothing() {
        let proxy = parse_dev_proxy("127.0.0.1:8899").expect("host:port");
        assert_eq!(proxy.protocol(), ProxyProtocol::Http);
        assert_eq!((proxy.host(), proxy.port()), ("127.0.0.1", 8899));
        assert!(!proxy.resolve_target(), "the proxy does the lookup");
        let local: ureq::http::Uri = "http://127.0.0.1:8545/".parse().unwrap();
        assert!(!proxy.is_no_proxy(&local));
        assert_eq!(
            parse_dev_proxy("localhost:3128").map(|p| p.port()),
            Some(3128)
        );
        for bad in [
            "",
            "127.0.0.1",
            "127.0.0.1:",
            ":8899",
            "127.0.0.1:0",
            "127.0.0.1:65536",
            "http://127.0.0.1:8899",
            "user@127.0.0.1:8899",
        ] {
            assert!(parse_dev_proxy(bad).is_none(), "{bad:?} was accepted");
        }
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

    // -- the walk (spec 082 T061) ---------------------------------------------

    fn dead_proxy() -> Route {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        Route::HttpConnect {
            host: "127.0.0.1".to_owned(),
            port: closed,
        }
    }

    /// Walk `routes`, each route answering with `answer(route)`; which routes
    /// were tried, in order, and the result.
    fn walked(
        routes: &[Route],
        mut answer: impl FnMut(&Route) -> Result<&'static str, ureq::Error>,
    ) -> (Vec<String>, Result<&'static str, Transport>) {
        let mut tried = Vec::new();
        let result = walk(routes, |route| {
            tried.push(route.label());
            answer(route)
        });
        (tried, result)
    }

    /// The one failure that moves a request on: the TCP connect to the proxy
    /// itself failed. The next route is tried and its answer stands.
    #[test]
    fn a_proxy_connect_failure_moves_on() {
        let dead = dead_proxy();
        let routes = [dead.clone(), Route::Direct];
        let (tried, result) = walked(&routes, |route| match route {
            Route::Direct => Ok("answered"),
            _ => Err(ureq::Error::Io(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        });
        assert_eq!(tried, vec![dead.label(), "direct".to_owned()]);
        assert_eq!(result.ok(), Some("answered"));

        // Every route's proxy down: the proxy's own failure, named.
        let (_, result) = walked(std::slice::from_ref(&dead), |_| {
            Err(ureq::Error::HostNotFound)
        });
        let failure = result
            .err()
            .unwrap_or_else(|| unreachable!("nothing answered"));
        assert!(failure.local);
        assert_eq!(
            failure.proxy,
            Some(ProxyFailure {
                proxy: dead.label(),
                kind: ProxyFailureKind::Unreachable,
            })
        );
        // A connect that timed out on a proxy that takes no connection is
        // that proxy timing out.
        let (tried, result) = walked(&routes, |route| match route {
            Route::Direct => Ok("answered"),
            _ => Err(ureq::Error::Timeout(ureq::Timeout::Connect)),
        });
        assert_eq!(tried.len(), 2);
        assert_eq!(result.ok(), Some("answered"));
    }

    /// A timeout after connecting, TLS, a CONNECT answered 5xx or closed with
    /// no reply: the proxy took the connection and spoke for the host. The
    /// request stops there — never a second route, never direct.
    #[test]
    fn a_failure_after_the_proxy_answered_does_not_move_on() {
        let dead = dead_proxy();
        let routes = [dead, Route::Direct];
        for (error, refused) in [
            (ureq::Error::Timeout(ureq::Timeout::Global), false),
            (ureq::Error::Timeout(ureq::Timeout::RecvResponse), false),
            (ureq::Error::Tls("handshake"), false),
            (
                ureq::Error::ConnectProxyFailed(
                    "proxy server responded 502/Bad Gateway".to_owned(),
                ),
                true,
            ),
            (
                ureq::Error::ConnectProxyFailed("proxy server did not respond".to_owned()),
                true,
            ),
        ] {
            let label = format!("{error:?}");
            let mut once = Some(error);
            let (tried, result) = walked(&routes, |_| {
                Err(once.take().unwrap_or(ureq::Error::ConnectionFailed))
            });
            assert_eq!(tried.len(), 1, "{label} moved on");
            let failure = result.err().unwrap_or_else(|| unreachable!("{label}"));
            assert_eq!(
                failure.proxy.map(|p| p.kind),
                refused.then_some(ProxyFailureKind::RefusedTunnel),
                "{label}"
            );
        }
        // A connect timeout on a proxy that DOES take connections is later
        // than the TCP connect (the CONNECT, TLS): not the proxy's.
        let live = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| unreachable!("loopback: {e}"));
        let port = live.local_addr().map(|addr| addr.port()).unwrap_or(0);
        let answering = Route::HttpConnect {
            host: "127.0.0.1".to_owned(),
            port,
        };
        let (tried, _) = walked(&[answering, Route::Direct], |_| {
            Err(ureq::Error::Timeout(ureq::Timeout::Connect))
        });
        assert_eq!(tried.len(), 1);
    }

    /// A PAC file that cannot run leaves no route: nothing is tried — above
    /// all not DIRECT — and the failure is the proxy's.
    #[test]
    fn a_pac_failure_is_a_proxy_failure_with_no_direct_fall_back() {
        let mut calls = 0;
        let result: Result<(), Transport> = over_routes(
            Err(ProxyFailure {
                proxy: "wpad.corp".to_owned(),
                kind: ProxyFailureKind::PacFailed,
            }),
            Duration::from_secs(1),
            |_| {
                calls += 1;
                Ok(())
            },
        );
        assert_eq!(calls, 0, "nothing went out");
        let failure = result.err().unwrap_or_else(|| unreachable!("no route"));
        assert!(failure.local);
        assert_eq!(
            failure.proxy.map(|p| p.kind),
            Some(ProxyFailureKind::PacFailed)
        );
        assert!(ProxyFailureKind::PacFailed.is_the_proxys());
        assert!(!ProxyFailureKind::RefusedTunnel.is_the_proxys());
    }

    /// No state survives between two requests: the second starts at the
    /// first route again, however the first one ended.
    #[test]
    fn no_state_survives_between_two_requests() {
        let dead = dead_proxy();
        let routes = [dead.clone(), Route::Direct];
        let refused = |route: &Route| match route {
            Route::Direct => Ok("answered"),
            _ => Err(ureq::Error::Io(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))),
        };
        let (first, _) = walked(&routes, &refused);
        let (second, _) = walked(&routes, &refused);
        assert_eq!(first, second);
        assert_eq!(second[0], dead.label());
    }

    /// The real thing: a proxy nobody listens on, then direct, over real
    /// agents — the request ends on the local server's answer.
    #[test]
    fn a_dead_proxy_then_direct_reaches_the_server() {
        let server = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| unreachable!("loopback: {e}"));
        let port = server.local_addr().map(|addr| addr.port()).unwrap_or(0);
        std::thread::spawn(move || {
            use std::io::{Read as _, Write as _};
            if let Some(Ok(mut stream)) = server.incoming().next() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
            }
        });
        let url = format!("http://127.0.0.1:{port}/");
        let body = over_routes(
            Ok(vec![dead_proxy(), Route::Direct]),
            Duration::from_secs(5),
            |agent| agent.get(&url).call()?.body_mut().read_to_string(),
        )
        .unwrap_or_else(|failure| unreachable!("{:?}", failure.error));
        assert_eq!(body, "ok");
    }

    /// Loopback never takes a proxy; everything else asks the platform.
    #[cfg(not(feature = "dev-fixtures"))]
    #[test]
    fn loopback_is_direct() {
        let routes = routes_for("http://127.0.0.1:8545/").unwrap_or_default();
        assert!(matches!(routes.as_slice(), [Route::Direct]));
    }
}
