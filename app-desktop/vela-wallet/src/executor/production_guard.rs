//! §28: a test build never writes to production.
//!
//! The rule is the core's (`vela_core::service_defaults::write_verdict`);
//! this is where the desktop enforces it. Every request the app makes goes
//! through an agent `proxy` builds (`agent_over`), and in a TEST build that
//! agent carries [`ProductionWriteGuard`]: a ureq middleware that asks the
//! core about each request — its method, its URL and, for a POST to a
//! production host, its body (a JSON-RPC read is a read) — before anything
//! is written to a socket. A refused request never leaves the machine: it
//! fails with [`refusal`]'s error, which `proxy::walk` reports as `local`
//! (not sent), and it is logged (`vlog!("guard", …)`).
//!
//! A test build is a debug build or a `dev-fixtures` build ([`TEST_BUILD`]);
//! `VELA_ALLOW_PRODUCTION_WRITES=1` ([`allow_production_writes`]) opts one
//! in. A release build — `cargo build --release`, every package, the Mac App
//! Store build — installs no guard at all ([`guard_for`] answers `None`), and
//! the switch's name is not even in its binary (`dev_env`,
//! `scripts/check-store-binary.sh`).

use std::io::Read as _;

use ureq::http;
use ureq::middleware::{Middleware, MiddlewareNext};
use ureq::{Body, SendBody};
use vela_core::service_defaults::{self as rule, WriteVerdict};

use crate::diag::vlog;

/// A debug build or a `dev-fixtures` build (`BuildFacts`: `debug`,
/// `dev_fixtures`). The desktop has no device-test flavour.
pub const TEST_BUILD: bool = cfg!(any(debug_assertions, feature = "dev-fixtures"));

/// `VELA_ALLOW_PRODUCTION_WRITES=1` — read only in a developer build.
#[must_use]
pub fn allow_production_writes() -> bool {
    rule::opt_in(crate::dev_env::var!("VELA_ALLOW_PRODUCTION_WRITES").as_deref())
}

/// The guard an agent carries: `None` for a release build, or a test build
/// that opted in — nothing to ask then.
#[must_use]
pub fn guard_for(test_build: bool, allow: bool) -> Option<ProductionWriteGuard> {
    (test_build && !allow).then_some(ProductionWriteGuard)
}

/// This build's guard ([`TEST_BUILD`], [`allow_production_writes`]).
#[must_use]
pub fn this_build() -> Option<ProductionWriteGuard> {
    guard_for(TEST_BUILD, allow_production_writes())
}

/// What a refused request fails with: an I/O error carrying [`Refused`].
fn refusal(host: &str) -> ureq::Error {
    ureq::Error::Io(std::io::Error::other(Refused {
        host: host.to_owned(),
    }))
}

/// The refusal, as the error's source: `proxy` recognises it
/// ([`is_refusal`]) and says the request was never sent.
#[derive(Debug)]
pub struct Refused {
    pub host: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", rule::REFUSED_ENGLISH, self.host)
    }
}

impl std::error::Error for Refused {}

/// Is `error` this guard's refusal.
#[must_use]
pub fn is_refusal(error: &ureq::Error) -> bool {
    matches!(error, ureq::Error::Io(io) if io.get_ref().is_some_and(|e| e.is::<Refused>()))
}

/// The ureq middleware (module note). Installed only in a test build.
#[derive(Clone, Copy, Debug)]
pub struct ProductionWriteGuard;

impl Middleware for ProductionWriteGuard {
    fn handle(
        &self,
        request: http::Request<SendBody>,
        next: MiddlewareNext,
    ) -> Result<http::Response<Body>, ureq::Error> {
        let method = request.method().as_str().to_owned();
        let url = request.uri().to_string();
        // Only a POST to a production host has a body worth reading (a
        // JSON-RPC read is allowed); every other request is decided on its
        // method and host alone, and its body is never touched.
        let candidate = method == "POST"
            && rule::url_host(&url).is_some_and(|host| rule::is_production_host(&host));
        if !candidate {
            return match rule::write_verdict(true, false, &method, &url, None) {
                WriteVerdict::Allowed => next.handle(request),
                WriteVerdict::Refused { host, .. } => {
                    vlog!("guard", "refused {method} {url}: {}", rule::REFUSED_ENGLISH);
                    Err(refusal(&host))
                }
            };
        }
        let (mut parts, body) = request.into_parts();
        let mut bytes = Vec::new();
        body.into_reader().read_to_end(&mut bytes)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        match rule::write_verdict(true, false, &method, &url, Some(&text)) {
            WriteVerdict::Refused { host, .. } => {
                vlog!("guard", "refused {method} {url}: {}", rule::REFUSED_ENGLISH);
                Err(refusal(&host))
            }
            WriteVerdict::Allowed => {
                // A read: sent as it was, its length said up front (a body
                // rebuilt from a reader would otherwise go chunked).
                parts
                    .headers
                    .insert(http::header::CONTENT_LENGTH, bytes.len().into());
                let mut reader = std::io::Cursor::new(bytes);
                next.handle(http::Request::from_parts(
                    parts,
                    SendBody::from_reader(&mut reader),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Proven with a local server and the production host names mapped to
    //! it: every request below goes to `127.0.0.1` through an HTTP proxy
    //! route the test owns, whatever host its URL names. Nothing here can
    //! reach a production service.

    use std::io::{BufRead as _, BufReader, Write as _};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::executor::proxy::{self, Route};

    /// A loopback "proxy" that answers every request itself — the production
    /// host names, mapped to this machine. Hands each request line it
    /// received (after a CONNECT, the tunnelled one) to `seen`.
    fn loopback() -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (seen, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = seen.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                    let mut writer = stream;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        let mut length = 0usize;
                        loop {
                            let mut header = String::new();
                            if reader.read_line(&mut header).unwrap_or(0) == 0 {
                                return;
                            }
                            if let Some(v) =
                                header.to_ascii_lowercase().strip_prefix("content-length:")
                            {
                                length = v.trim().parse().unwrap_or(0);
                            }
                            if header == "\r\n" {
                                break;
                            }
                        }
                        if line.starts_with("CONNECT ") {
                            let _ =
                                writer.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                            continue;
                        }
                        let mut body = vec![0; length];
                        let _ = std::io::Read::read_exact(&mut reader, &mut body);
                        // An absolute-form line (a proxy's) is read as the
                        // origin-form one the host itself would get.
                        let line = line.trim().replacen("http://", "", 1);
                        let line = match line.split_once(' ') {
                            Some((method, rest)) if !rest.starts_with('/') => {
                                let rest = rest.find('/').map_or(rest, |at| &rest[at..]);
                                format!("{method} {rest}")
                            }
                            _ => line,
                        };
                        let _ = seen.send(line);
                        let answer = b"{\"ok\":true}";
                        let _ = write!(
                            writer,
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                            answer.len()
                        );
                        let _ = writer.write_all(answer);
                        return;
                    }
                });
            }
        });
        (port, rx)
    }

    fn agent(port: u16, guard: Option<ProductionWriteGuard>) -> ureq::Agent {
        proxy::agent_with(
            &Route::HttpConnect {
                host: "127.0.0.1".to_owned(),
                port,
            },
            Duration::from_secs(5),
            guard,
        )
    }

    /// The key index's registration — the round-17 write. `http://`, so the
    /// loopback server needs no certificate for the production name.
    const REGISTER: &str = "http://p256-index-v2.getvela.app/api/register";

    #[test]
    fn a_test_build_refuses_a_production_write_before_it_leaves() {
        let (port, seen) = loopback();
        let result = agent(port, guard_for(true, false))
            .post(REGISTER)
            .send_json(serde_json::json!({ "members": [] }));
        let error = result.expect_err("refused");
        assert!(is_refusal(&error), "{error}");
        assert!(error.to_string().contains(rule::REFUSED_ENGLISH), "{error}");
        assert!(
            seen.recv_timeout(Duration::from_millis(300)).is_err(),
            "nothing reached the (loopback) index"
        );
    }

    #[test]
    fn a_test_build_still_reads_production() {
        let (port, seen) = loopback();
        let agent = agent(port, guard_for(true, false));
        agent
            .get("http://p256-index-v2.getvela.app/api/health")
            .call()
            .expect("a read goes out");
        assert_eq!(seen.recv().expect("seen"), "GET /api/health HTTP/1.1");
        // A JSON-RPC read to the relay is a read: sent whole, body and all.
        let mut answer = agent
            .post("http://vela-relay-cf.getvela.app/100")
            .send_json(serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "eth_estimateUserOperationGas", "params": []
            }))
            .expect("a JSON-RPC read goes out");
        assert_eq!(
            answer.body_mut().read_to_string().expect("body"),
            "{\"ok\":true}"
        );
        assert_eq!(seen.recv().expect("seen"), "POST /100 HTTP/1.1");
        // A submit to the same relay is a write.
        let submit = agent
            .post("http://vela-relay-cf.getvela.app/100")
            .send_json(serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "eth_sendUserOperation", "params": []
            }));
        assert!(is_refusal(&submit.expect_err("refused")));
    }

    #[test]
    fn a_test_build_writes_to_a_local_stand_in() {
        let (port, seen) = loopback();
        agent(port, guard_for(true, false))
            .post(format!("http://127.0.0.1:{port}/api/register"))
            .send_json(serde_json::json!({}))
            .expect("a local write goes out");
        assert_eq!(seen.recv().expect("seen"), "POST /api/register HTTP/1.1");
    }

    #[test]
    fn the_opt_in_lets_a_test_build_write() {
        assert!(guard_for(true, true).is_none());
        let (port, seen) = loopback();
        agent(port, guard_for(true, true))
            .post(REGISTER)
            .send_json(serde_json::json!({}))
            .expect("opted in");
        assert_eq!(seen.recv().expect("seen"), "POST /api/register HTTP/1.1");
    }

    /// A release build carries no guard, and its write leaves exactly as
    /// before — here to the loopback stand-in for the production name.
    #[test]
    fn a_release_build_is_unaffected() {
        assert!(guard_for(false, false).is_none());
        assert!(guard_for(false, true).is_none());
        let (port, seen) = loopback();
        agent(port, guard_for(false, false))
            .post(REGISTER)
            .send_json(serde_json::json!({ "members": [] }))
            .expect("a release build's write goes out");
        assert_eq!(seen.recv().expect("seen"), "POST /api/register HTTP/1.1");
    }

    /// The build this test binary is: a debug build is a test build.
    #[test]
    #[cfg(debug_assertions)]
    fn this_debug_build_is_a_test_build() {
        const { assert!(TEST_BUILD) };
        assert!(this_build().is_some() || allow_production_writes());
    }

    /// `cargo test --release` (without `dev-fixtures`): the build a store
    /// package is, and it installs no guard.
    #[test]
    #[cfg(all(not(debug_assertions), not(feature = "dev-fixtures")))]
    fn this_release_build_installs_no_guard() {
        const { assert!(!TEST_BUILD) };
        assert!(this_build().is_none());
    }

    /// The shared vector, replayed through the core's door the guard asks.
    #[test]
    fn the_vector_answers_as_the_core_says() {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../rust/crates/vela-core/tests/vectors/rules/production-writes.json"
        ))
        .expect("vector");
        for case in v["cases"].as_array().expect("cases") {
            let verdict = rule::write_verdict(
                case["testBuild"].as_bool().unwrap_or_default(),
                case["allowProductionWrites"].as_bool().unwrap_or_default(),
                case["method"].as_str().unwrap_or_default(),
                case["url"].as_str().unwrap_or_default(),
                case["body"].as_str(),
            );
            let refused = match verdict {
                WriteVerdict::Refused { host, .. } => Some(host),
                WriteVerdict::Allowed => None,
            };
            assert_eq!(
                refused.as_deref(),
                case["host"].as_str(),
                "{}",
                case["about"]
            );
        }
    }
}
