//! R22 (§36): local JSON-RPC proxies in front of the fork harness's anvil —
//! an HONEST one (forwards as it is) and a LYING one (rewrites what a
//! signing verdict stands on to "nothing happens"). Each listens on its own
//! loopback port, so the pool counts it as its own provider
//! (`corroborate::provider_of`: a loopback address is its port).
//!
//! The lie, per method:
//! - `eth_simulateV1`: every call's logs dropped — "No asset changes";
//! - `debug_traceCall` (prestate, diff mode): every storage write dropped but
//!   a Safe's nonce (slot 5) — "nothing about who controls it changes";
//! - the state probe (`eth_call` of `simulateAndRevert`): refused, so the
//!   only state reading it gives is its lying trace.
//!
//! Everything else is forwarded untouched, `eth_chainId` included: the liar
//! serves the right chain.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

/// How a proxy treats the answers it forwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Honest,
    Lying,
    /// §39 (R22 follow-up): lies about WHO CAN ACT — every `eth_getCode`
    /// answered the other way round (code where there is none, none where
    /// there is) and every `getOwners()` answered with an attacker alone.
    /// Beside an honest node it is a disagreement, never a plain read.
    AuthorityLiar,
}

/// `getOwners()`.
const GET_OWNERS: &str = "0xa0e67e2b";

/// §39: the authority lie ([`Kind::AuthorityLiar`]).
pub fn lie_about_authority(request: &Value, mut answer: Value) -> Value {
    let method = request["method"].as_str().unwrap_or_default();
    let data = request["params"][0]["data"].as_str().unwrap_or_default();
    match method {
        "eth_getCode" => {
            let none = answer["result"].as_str().is_none_or(|c| c == "0x");
            answer["result"] = Value::String(if none { "0x6080604052" } else { "0x" }.to_owned());
        }
        "eth_call" if data.to_ascii_lowercase().starts_with(GET_OWNERS) => {
            answer["result"] = Value::String(format!(
                "0x{:064x}{:064x}{:0>64}",
                32, 1, "bad0000000000000000000000000000000000bad"
            ));
        }
        _ => {}
    }
    answer
}

/// A running proxy: its URL.
pub struct Proxy {
    pub url: String,
}

/// One HTTP request in, its JSON body (the length read from the head).
fn read_request(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf).into_owned();
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let len = head
                .lines()
                .find_map(|l| {
                    let l = l.to_ascii_lowercase();
                    l.strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if body.len() >= len {
                return Some(body[..len].to_owned());
            }
        }
    }
}

/// POST `body` to `url` (plain http, loopback): the answer's body.
pub fn post(url: &str, body: &str) -> Option<String> {
    let rest = url.strip_prefix("http://")?;
    let (host, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let mut s = TcpStream::connect(host).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, payload) = text.split_once("\r\n\r\n")?;
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut out = String::new();
        let mut rest = payload;
        while let Some((size, tail)) = rest.split_once("\r\n") {
            let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
            if n == 0 {
                break;
            }
            out.push_str(&tail[..n]);
            rest = &tail[n + 2..];
        }
        return Some(out);
    }
    Some(payload.to_owned())
}

const NONCE_SLOT: &str = "0x0000000000000000000000000000000000000000000000000000000000000005";

/// `simulateAndRevert(address,bytes)` — the state probe's call.
const PROBE_SELECTOR: &str = "0xb4faba09";

/// The lie told about one answer (module doc); `request` is what was asked.
pub fn lie(request: &Value, mut answer: Value) -> Value {
    let method = request["method"].as_str().unwrap_or_default();
    // The state probe (`simulateAndRevert` + SafeStateProbe): answered as if
    // the node could not run it — no reading of what changes.
    let data = request["params"][0]["data"].as_str().unwrap_or_default();
    if method == "eth_call" && data.to_ascii_lowercase().starts_with(PROBE_SELECTOR) {
        return json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "execution reverted"}});
    }
    match method {
        "eth_simulateV1" => {
            if let Some(blocks) = answer["result"].as_array_mut() {
                for block in blocks {
                    if let Some(calls) = block["calls"].as_array_mut() {
                        for call in calls {
                            call["logs"] = json!([]);
                        }
                    }
                }
            }
        }
        "debug_traceCall" => {
            for side in ["pre", "post"] {
                if let Some(accounts) = answer["result"][side].as_object_mut() {
                    for account in accounts.values_mut() {
                        if let Some(storage) = account["storage"].as_object_mut() {
                            storage.retain(|slot, _| slot.eq_ignore_ascii_case(NONCE_SLOT));
                        }
                    }
                }
            }
        }
        _ => {}
    }
    answer
}

impl Proxy {
    /// A proxy of `kind` in front of `upstream`, on a fresh loopback port.
    pub fn start(upstream: &str, kind: Kind) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().unwrap());
        let upstream = upstream.to_owned();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let upstream = upstream.clone();
                thread::spawn(move || {
                    let Some(body) = read_request(&mut stream) else {
                        return;
                    };
                    let request = serde_json::from_str::<Value>(&body).unwrap_or_default();
                    let answer = post(&upstream, &body).unwrap_or_else(|| {
                        json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "upstream down"}})
                            .to_string()
                    });
                    let answer = match kind {
                        Kind::Honest => answer,
                        Kind::Lying => serde_json::from_str::<Value>(&answer)
                            .map(|v| lie(&request, v).to_string())
                            .unwrap_or(answer),
                        Kind::AuthorityLiar => serde_json::from_str::<Value>(&answer)
                            .map(|v| lie_about_authority(&request, v).to_string())
                            .unwrap_or(answer),
                    };
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                        answer.len()
                    );
                    let _ = stream.write_all(reply.as_bytes());
                });
            }
        });
        Proxy { url }
    }
}
