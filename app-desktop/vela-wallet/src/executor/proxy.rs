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
//! the system setting, the environment, direct — and [`with_candidates`]
//! walks it: a request whose failure is a transport failure is retried once
//! on the next candidate, and only when the last one refuses is the failure
//! reported, with the one bit the screen needs: did it fail to get out of
//! THIS MACHINE (`local`), or did a route exist and the far end not answer?
//! The list is re-derived after any failure and every [`REDERIVE_AFTER`], so
//! a proxy that comes back or a setting that changes is honoured without a
//! restart. Contract: `specs/038-first-run-parity/contracts/proxy-candidates.md`.
//!
//! What is deliberately NOT handled: `mode = 'auto'` (a PAC URL is a JavaScript
//! program, and running one to reach the key registry is not a trade this app
//! makes) and KDE's `kioslaverc`. Both fall through to no proxy, which is the
//! behaviour before this module existed.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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

/// Is this url on this machine (or its own network's name for it)?
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
    current: usize,
    derived_at: Instant,
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

/// The candidate every request goes through right now.
fn current() -> Candidate {
    let Ok(mut guard) = state().lock() else {
        return Candidate::Direct;
    };
    let stale = guard
        .as_ref()
        .is_none_or(|c| c.derived_at.elapsed() >= REDERIVE_AFTER);
    if stale {
        *guard = Some(Candidates {
            list: derive_candidates(),
            current: 0,
            derived_at: Instant::now(),
        });
    }
    guard
        .as_ref()
        .and_then(|c| c.list.get(c.current).cloned())
        .unwrap_or(Candidate::Direct)
}

/// A request through the current candidate could not get through. Move to
/// the next one; `false` when there is none left — the list is then dropped
/// so the next request derives it afresh rather than sitting on a dead end.
fn advance() -> bool {
    let Ok(mut guard) = state().lock() else {
        return false;
    };
    let Some(candidates) = guard.as_mut() else {
        return false;
    };
    if candidates.current + 1 < candidates.list.len() {
        candidates.current += 1;
        true
    } else {
        *guard = None;
        false
    }
}

/// The proxy to configure on an agent right now, or `None` for direct.
///
/// Kept as the module's one public read of the decision (the caBLE tunnel
/// dial logs it); everything else should go through [`with_candidates`] so a
/// refused route is retried rather than reported.
pub fn system_proxy() -> Option<Proxy> {
    current().proxy().cloned()
}

/// A transport failure, after every candidate was tried.
#[derive(Debug)]
pub struct Transport {
    /// The last candidate's error, verbatim.
    pub error: ureq::Error,
    /// Every route refused inside this machine: the proxies would not take
    /// the connection and a direct attempt could not even resolve or route
    /// to the host. `false` means a route existed and the far end did not
    /// answer — which may still be the person's network, but is not
    /// something a different proxy fixes.
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

/// Run one request over the candidate chain.
///
/// `call` is invoked with an agent for the current candidate; a transport
/// failure advances to the next candidate and calls it again, once per
/// candidate. The first `Ok` — or the first non-transport `Err`, which is the
/// server talking — is returned as-is. Loopback targets never take a proxy
/// (see [`agent_for`]); callers with such a url should use that instead.
pub fn with_candidates<T>(
    timeout: Duration,
    mut call: impl FnMut(&Agent) -> Result<T, ureq::Error>,
) -> Result<T, Transport> {
    loop {
        let candidate = current();
        let agent = agent_over(candidate.proxy(), timeout);
        match call(&agent) {
            Ok(value) => return Ok(value),
            Err(error) if is_transport(&error) => {
                let was_direct = candidate.proxy().is_none();
                if advance() {
                    continue;
                }
                return Err(Transport {
                    local: was_direct && failed_inside_this_machine(&error),
                    error,
                });
            }
            Err(error) => {
                return Err(Transport {
                    error,
                    local: false,
                });
            }
        }
    }
}

/// [`with_candidates`] for ONE url: a loopback target never takes a proxy
/// (the [`agent_for`] rule), so it runs direct and once; everything else
/// walks the chain. The pool's per-endpoint calls go through here (T028).
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
    with_candidates(timeout, call)
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
            *state = Some(Candidates {
                list,
                current: 0,
                derived_at: Instant::now(),
            });
        }
        guard
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
        let body = with_candidates(Duration::from_secs(5), |agent| {
            agent.get(&url).call()?.body_mut().read_to_string()
        })
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
        let failure = with_candidates(Duration::from_secs(5), |agent| {
            agent
                .get("http://vela-038-no-such-host.invalid/")
                .call()
                .map(|_| ())
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
