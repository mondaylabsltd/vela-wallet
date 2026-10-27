//! Connecting the AI agent with clientmcp.dev on the desktop (spec 106 D4):
//! the loopback return of RFC 8252 §7.3. The protocol's every decision is
//! the core's (`vela_core::clientmcp_link`); this is the desktop's part —
//! a one-time listener on `127.0.0.1` at a port the system picks, the
//! browser opened at the site, and the one HTTP request that trades the code.
//!
//! No URL scheme is registered and the installers do not change: the
//! browser comes back to this process directly, whichever desktop it is.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use vela_core::clientmcp_link::{self, LinkError, Linked, Pending, RETURN_PATH};

use crate::executor::{proxy, storage};

/// clientmcp.dev, unless a development build names another
/// (`VELA_CLIENTMCP_SITE=https://staging.clientmcp.dev cargo build`).
pub fn site() -> &'static str {
    option_env!("VELA_CLIENTMCP_SITE").unwrap_or("https://clientmcp.dev")
}

/// What clientmcp.dev lists this installation as (the person can rename it there).
pub fn label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "windows") {
        "Windows PC"
    } else {
        "Linux PC"
    }
}

const KEY_INSTALLATION: &str = "vela.agent.installation";

/// This installation's ID: made once, kept beside the agent's settings.
fn installation() -> String {
    if let Ok(Some(serde_json::Value::String(kept))) = storage::read_value(KEY_INSTALLATION)
        && !kept.is_empty()
    {
        return kept;
    }
    let made = clientmcp_link::installation_id(&random::<16>());
    let _ = storage::write_value(KEY_INSTALLATION, serde_json::Value::String(made.clone()));
    made
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    // The system's generator failing is not something to sign in through.
    getrandom::fill(&mut bytes).expect("the system's random generator");
    bytes
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Why connecting did not work: the core's reasons, and the two only the
/// desktop can have — the network, and the person cancelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Link(LinkError),
    Network,
    Cancelled,
}

/// A sign-in under way: the browser is at the site, the listener waits.
pub struct Started {
    pub pending: Pending,
    listener: TcpListener,
    pub cancel: Arc<AtomicBool>,
}

/// Listens on a fresh loopback port and prepares the request; the caller
/// opens `pending.authorize_url` in the browser.
pub fn start() -> Result<Started, Problem> {
    start_at(site())
}

/// [`start`] against `site` (a test's own).
fn start_at(site: &str) -> Result<Started, Problem> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| Problem::Network)?;
    let port = listener.local_addr().map_err(|_| Problem::Network)?.port();
    listener
        .set_nonblocking(true)
        .map_err(|_| Problem::Network)?;
    let pending = clientmcp_link::begin(
        site,
        &clientmcp_link::loopback_redirect(port),
        label(),
        &installation(),
        &random::<{ clientmcp_link::RANDOM_LEN }>(),
        now_ms(),
    )
    .map_err(Problem::Link)?;
    Ok(Started {
        pending,
        listener,
        cancel: Arc::new(AtomicBool::new(false)),
    })
}

/// Blocking, on a background thread: waits for the browser's return (at most
/// [`clientmcp_link::PENDING_MS`]), answers it with `done_page`, and trades
/// the code. The listener closes when this returns.
pub fn finish(started: Started, done_page: &str) -> Result<Linked, Problem> {
    let Started {
        pending,
        listener,
        cancel,
    } = started;
    let deadline = Instant::now() + Duration::from_millis(clientmcp_link::PENDING_MS);
    let url = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Problem::Cancelled);
        }
        if Instant::now() > deadline {
            return Err(Problem::Link(LinkError::Expired));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(url) = answer(stream, &pending.redirect_uri, done_page) {
                    break url;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => return Err(Problem::Network),
        }
    };
    drop(listener);
    let code = clientmcp_link::returned(&pending, &url, now_ms()).map_err(Problem::Link)?;
    let request = clientmcp_link::exchange(&pending, &code);
    let (status, body) = post(&request.url, &request.body)?;
    clientmcp_link::accept(status, &body).map_err(Problem::Link)
}

/// One request to the listener: the return (answered with `done_page`, its
/// full address given back), or anything else (a favicon: 404, keep waiting).
fn answer(mut stream: TcpStream, redirect_uri: &str, done_page: &str) -> Option<String> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let target = target_of(&head);
    let ours = target.is_some_and(|t| t.split('?').next() == Some(RETURN_PATH));
    let (status, body) = if ours {
        ("200 OK", page(done_page))
    } else {
        ("404 Not Found", String::new())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\nreferrer-policy: no-referrer\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
    let origin = redirect_uri.strip_suffix(RETURN_PATH)?;
    ours.then(|| format!("{origin}{}", target.unwrap_or_default()))
}

/// `GET <target> HTTP/1.1` → `<target>`.
fn target_of(head: &str) -> Option<&str> {
    let line = head.lines().next()?;
    let mut parts = line.split(' ');
    (parts.next()? == "GET").then_some(())?;
    parts.next()
}

fn page(message: &str) -> String {
    let text = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>Vela</title><body style=\"font:16px system-ui;margin:20vh auto;max-width:28em;padding:0 1em;text-align:center\"><p>{text}</p>"
    )
}

/// The exchange, through the system's proxy like the rest of the app. A
/// refusal's body is wanted too (the site says why), so no status is an error.
fn post(url: &str, body: &str) -> Result<(u16, String), Problem> {
    let mut response = proxy::with_routes(url, Duration::from_secs(30), |agent| {
        agent
            .post(url)
            .config()
            .http_status_as_error(false)
            .build()
            .header("content-type", "application/json")
            .send(body)
    })
    .map_err(|_| Problem::Network)?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    Ok((status, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listener_answers_its_return_and_ignores_the_rest() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let redirect = clientmcp_link::loopback_redirect(port);
        let ask = |path: &str| {
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(client, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
            client
        };
        let read = |mut client: TcpStream| {
            let mut out = String::new();
            client.read_to_string(&mut out).unwrap();
            out
        };

        let favicon = ask("/favicon.ico");
        let (stream, _) = listener.accept().unwrap();
        assert_eq!(answer(stream, &redirect, "x"), None);
        assert!(read(favicon).starts_with("HTTP/1.1 404"));

        let back = ask("/clientmcp?code=c&state=s");
        let (stream, _) = listener.accept().unwrap();
        assert_eq!(
            answer(stream, &redirect, "Done <here> & now").as_deref(),
            Some(format!("http://127.0.0.1:{port}/clientmcp?code=c&state=s").as_str())
        );
        let page = read(back);
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.contains("Done &lt;here&gt; &amp; now"));
    }

    /// The whole trip, with clientmcp.dev played on a loopback port and the
    /// browser's return made by hand: the listener takes it, the code and
    /// the verifier go to the site, and its answer is the result.
    #[test]
    fn the_return_is_traded_for_the_bridge_and_the_token() {
        storage::tests::with_temp_state("agent-link-trip", || {
            let site = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let site_url = format!("http://127.0.0.1:{}", site.local_addr().unwrap().port());
            let started = start_at(&site_url).unwrap();
            let state = started.pending.state.clone();
            let verifier = started.pending.verifier.clone();
            let redirect = started.pending.redirect_uri.clone();
            let played = std::thread::spawn(move || {
                let (mut stream, _) = site.accept().unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                // Head and body: until the JSON body is in.
                while !String::from_utf8_lossy(&request).contains("redirect_uri") {
                    let n = stream.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                }
                let body = r#"{"client_id":"c-1","client_token":"issued","expires_at":1830000000,"bridge_url":"wss://bridge.example/connect","agent_url":"https://bridge.example/mcp/agent-tok","login":"alice"}"#;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
                String::from_utf8_lossy(&request).into_owned()
            });
            // The browser, coming back with the code.
            let back = redirect
                .strip_prefix("http://")
                .unwrap()
                .split('/')
                .next()
                .unwrap()
                .to_owned();
            let browser = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                let mut client = TcpStream::connect(back).unwrap();
                write!(
                    client,
                    "GET /clientmcp?code=the-code&state={state} HTTP/1.1\r\n\r\n"
                )
                .unwrap();
                let mut page = String::new();
                client.read_to_string(&mut page).unwrap();
                page
            });
            let linked = finish(started, "Connected").unwrap();
            assert_eq!(linked.login, "alice");
            assert_eq!(linked.client_token, "issued");
            assert_eq!(
                linked.agent_url.as_deref(),
                Some("https://bridge.example/mcp/agent-tok")
            );
            assert!(browser.join().unwrap().contains("Connected"));
            let request = played.join().unwrap();
            assert!(request.starts_with("POST /api/token "), "{request}");
            assert!(request.contains("\"code\":\"the-code\""));
            assert!(request.contains(&format!("\"code_verifier\":\"{verifier}\"")));
        });
    }

    #[test]
    fn a_cancelled_sign_in_stops_waiting() {
        storage::tests::with_temp_state("agent-link-cancel", || {
            let started = start().unwrap();
            assert!(
                started
                    .pending
                    .redirect_uri
                    .starts_with("http://127.0.0.1:")
            );
            assert!(started.pending.authorize_url.starts_with(site()));
            started.cancel.store(true, Ordering::Relaxed);
            assert_eq!(finish(started, "x"), Err(Problem::Cancelled));
            // The installation is made once and kept.
            let first = installation();
            assert_eq!(installation(), first);
        });
    }
}
