//! The phones' channel to the Clear Signer: a WebSocket on the wallet's own
//! loopback (spec 071).
//!
//! The shell owns a TCP listener on `127.0.0.1` and nothing else: it hands
//! every byte a connection sends to a [`Connection`], writes back the bytes
//! the connection returns, and closes the socket when told to. The handshake
//! (RFC 6455 §4.2), the frames (§5), the page's `Origin`, the one-time token
//! and the message order all live here, so no shell frames a WebSocket or
//! decides who may talk to it.
//!
//! One connection, one conversation:
//!
//! ```text
//!   page → GET / Upgrade: websocket, Origin: <the signer page's origin>
//!   app  → 101
//!   page → {v:1, t:"hello", token}           wrong token → closed, keep listening
//!   app  → {v:1, t:"intent", id, intent, context}
//!   page → {v:1, t:"result", id, result}     → the outcome: the answer, to verify
//!        | {v:1, t:"error", id, code}        → the outcome: a refusal
//!        | the socket closes                 → the outcome: closed without signing
//! ```
//!
//! A connection that never proved itself (another page on the device found
//! the port, an old tab reconnected with a spent token) is closed and has no
//! outcome; the shell keeps listening until one connection has one.
//!
//! Spec 075: a conversation may hold several requests. After an answer the
//! connection waits, open, for the shell's next [`Connection::send`] (a
//! create's member proof, a recovery's second signature) or its
//! [`Connection::end`] (`{t:"bye"}`, then close). A page that closes between
//! requests ends the session without an outcome.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use sha1::{Digest as _, Sha1};

use super::{refusal, ClearSignerError};

/// The one message the wallet sends once the page has said hello with the
/// right token.
pub fn intent_message(id: &str, request: &Value) -> String {
    json!({
        "v": 1,
        "t": "intent",
        "id": id,
        "intent": request.get("intent").cloned().unwrap_or(Value::Null),
        "context": request.get("context").cloned().unwrap_or(Value::Null),
    })
    .to_string()
}

/// A message from the page.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// `{t:"hello", token}` — the page is ready; the token proves it was
    /// opened by this wallet for this request.
    Hello {
        token: String,
    },
    /// `{t:"result", id, result}` — the assertion, still to be verified.
    Result {
        id: String,
        result: Value,
    },
    /// `{t:"error", id, code}`.
    Error {
        id: String,
        error: ClearSignerError,
    },
    Ignored,
}

pub fn parse_message(text: &str) -> Message {
    let Ok(Value::Object(message)) = serde_json::from_str::<Value>(text) else {
        return Message::Ignored;
    };
    let text_of = |name: &str| {
        message
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    match message.get("t").and_then(Value::as_str) {
        Some("hello") => Message::Hello {
            token: text_of("token"),
        },
        Some("result") => match message.get("result") {
            Some(result) if result.is_object() => Message::Result {
                id: text_of("id"),
                result: result.clone(),
            },
            // Spec 075: a ceremony's answer carries `registration` or
            // `assertion` beside `id`; the verifier reads the whole message.
            _ if message.contains_key("registration") || message.contains_key("assertion") => {
                Message::Result {
                    id: text_of("id"),
                    result: Value::Object(message.clone()),
                }
            }
            _ => Message::Ignored,
        },
        Some("error") => Message::Error {
            id: text_of("id"),
            error: refusal(&text_of("code")),
        },
        _ => Message::Ignored,
    }
}

/// What to do after feeding a connection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// Bytes to write to the socket, in order.
    pub write: Vec<u8>,
    /// Close the socket once `write` is written.
    pub close: bool,
    /// The conversation's end — at most once per connection: the page's
    /// answer (still to be verified against the digest) or why there is none.
    pub outcome: Option<Result<Value, ClearSignerError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Handshake,
    Hello,
    Answer,
    /// Spec 075: answered; open for the shell's next request or its end.
    Idle,
    Done,
}

/// The largest message a page may send: an assertion is a few KiB.
const MAX_MESSAGE: usize = 256 * 1024;
/// The largest HTTP head before the upgrade.
const MAX_HEAD: usize = 16 * 1024;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// One accepted TCP connection.
pub struct Connection {
    origin: String,
    token: String,
    id: String,
    intent: String,
    phase: Phase,
    inbox: Vec<u8>,
    /// A fragmented text message being joined.
    partial: Vec<u8>,
}

impl Connection {
    /// `signer_url` is the page the wallet opened; its origin is the only
    /// `Origin` accepted. `request` is [`super::request`]'s `{intent, context}`.
    pub fn new(signer_url: &str, token: &str, id: &str, request: &Value) -> Self {
        Self {
            origin: origin_of(signer_url),
            token: token.to_owned(),
            id: id.to_owned(),
            intent: intent_message(id, request),
            phase: Phase::Handshake,
            inbox: Vec::new(),
            partial: Vec::new(),
        }
    }

    /// Bytes arrived on the socket.
    pub fn feed(&mut self, bytes: &[u8]) -> Step {
        let mut step = Step::default();
        if self.phase == Phase::Done {
            return step;
        }
        self.inbox.extend_from_slice(bytes);
        if self.phase == Phase::Handshake {
            let Some(end) = find(&self.inbox, b"\r\n\r\n") else {
                if self.inbox.len() > MAX_HEAD {
                    return self.refuse_http(step);
                }
                return step;
            };
            let head = String::from_utf8_lossy(&self.inbox[..end]).into_owned();
            self.inbox.drain(..end + 4);
            match accept(&head, &self.origin) {
                Some(response) => {
                    step.write.extend_from_slice(response.as_bytes());
                    self.phase = Phase::Hello;
                }
                None => return self.refuse_http(step),
            }
        }
        while self.phase != Phase::Done {
            let Some((frame, used)) = decode(&self.inbox) else {
                if self.inbox.len() > MAX_MESSAGE + 14 {
                    self.fail(&mut step, 1009);
                }
                break;
            };
            self.inbox.drain(..used);
            match frame {
                Frame::Bad => {
                    self.fail(&mut step, 1002);
                }
                Frame::Ping(payload) => step.write.extend(encode(0xA, &payload)),
                Frame::Pong | Frame::Binary => {}
                Frame::Close => {
                    if self.phase == Phase::Answer {
                        step.outcome = Some(Err(ClearSignerError::Declined));
                    }
                    step.write.extend(close_frame(1000));
                    step.close = true;
                    self.phase = Phase::Done;
                }
                Frame::Text { fin, payload } => {
                    self.partial.extend_from_slice(&payload);
                    if self.partial.len() > MAX_MESSAGE {
                        self.fail(&mut step, 1009);
                        continue;
                    }
                    if fin {
                        let text = String::from_utf8_lossy(&std::mem::take(&mut self.partial))
                            .into_owned();
                        self.message(&text, &mut step);
                    }
                }
            }
        }
        step
    }

    /// The socket closed (or broke) under the connection. An accepted page
    /// that goes away without answering was closed without signing; one that
    /// never proved itself changes nothing.
    pub fn closed(&mut self) -> Option<ClearSignerError> {
        let phase = std::mem::replace(&mut self.phase, Phase::Done);
        (phase == Phase::Answer).then_some(ClearSignerError::Declined)
    }

    /// Spec 075: the next request of the same session, once the last one has
    /// its answer. Nothing to write in any other phase — a shell that sends
    /// early, or after the end, is told so by an empty step.
    pub fn send(&mut self, id: &str, request: &Value) -> Step {
        let mut step = Step::default();
        if self.phase == Phase::Idle {
            self.id = id.to_owned();
            step.write
                .extend(encode(0x1, intent_message(id, request).as_bytes()));
            self.phase = Phase::Answer;
        }
        step
    }

    /// Spec 075: the session is over — `{t:"bye"}`, then close. The page
    /// leaves its waiting screen for its done screen.
    pub fn end(&mut self) -> Step {
        let mut step = Step::default();
        if self.phase != Phase::Done && self.phase != Phase::Handshake {
            let bye = json!({ "v": 1, "t": "bye", "reason": "done" }).to_string();
            step.write.extend(encode(0x1, bye.as_bytes()));
        }
        step.write.extend(close_frame(1000));
        step.close = true;
        self.phase = Phase::Done;
        step
    }

    fn message(&mut self, text: &str, step: &mut Step) {
        match (self.phase, parse_message(text)) {
            (Phase::Hello, Message::Hello { token }) => {
                if same(&token, &self.token) {
                    step.write.extend(encode(0x1, self.intent.as_bytes()));
                    self.phase = Phase::Answer;
                } else {
                    self.fail(step, 1008);
                }
            }
            // An answer leaves the session open for the next request (spec
            // 075); the shell ends it when the flow is done.
            (Phase::Answer, Message::Result { id, result }) if id == self.id => {
                step.outcome = Some(Ok(result));
                self.phase = Phase::Idle;
            }
            (Phase::Answer, Message::Error { id, error }) if id == self.id => {
                step.outcome = Some(Err(error));
                self.phase = Phase::Idle;
            }
            _ => {}
        }
    }

    /// A protocol fault. An accepted page that breaks the protocol ends the
    /// conversation as refused, not as a silent hang.
    fn fail(&mut self, step: &mut Step, code: u16) {
        if self.phase == Phase::Answer {
            step.outcome = Some(Err(ClearSignerError::Malformed(format!(
                "websocket close {code}"
            ))));
        }
        step.write.extend(close_frame(code));
        step.close = true;
        self.phase = Phase::Done;
    }

    fn refuse_http(&mut self, mut step: Step) -> Step {
        step.write.extend_from_slice(
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        step.close = true;
        self.phase = Phase::Done;
        step
    }
}

/// `scheme://host[:port]` of a URL, lowercased — what a browser sends as
/// `Origin`.
pub fn origin_of(url: &str) -> String {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once("://") else {
        return String::new();
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or_default();
    let scheme = scheme.to_ascii_lowercase();
    let authority = authority.to_ascii_lowercase();
    // The default port is never written in an Origin.
    let authority = match (scheme.as_str(), authority.rsplit_once(':')) {
        ("https", Some((host, "443"))) | ("http", Some((host, "80"))) => host.to_owned(),
        _ => authority,
    };
    format!("{scheme}://{authority}")
}

/// The 101 for a WebSocket upgrade from `origin`, or `None`.
fn accept(head: &str, origin: &str) -> Option<String> {
    let mut lines = head.split("\r\n");
    let request = lines.next()?;
    if !request.starts_with("GET ") {
        return None;
    }
    let mut key = None;
    let mut upgrade = false;
    let mut from = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "upgrade" => upgrade = value.eq_ignore_ascii_case("websocket"),
            "sec-websocket-key" => key = Some(value.to_owned()),
            "origin" => from = Some(value.to_ascii_lowercase()),
            _ => {}
        }
    }
    if !upgrade || origin.is_empty() || from.as_deref() != Some(origin) {
        return None;
    }
    let mut hash = Sha1::new();
    hash.update(key?.as_bytes());
    hash.update(GUID.as_bytes());
    let accept = STANDARD.encode(hash.finalize());
    Some(format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    ))
}

enum Frame {
    Text { fin: bool, payload: Vec<u8> },
    Binary,
    Close,
    Ping(Vec<u8>),
    Pong,
    Bad,
}

/// One client frame from the front of `bytes`, and how many bytes it used.
fn decode(bytes: &[u8]) -> Option<(Frame, usize)> {
    if bytes.len() < 2 {
        return None;
    }
    let fin = bytes[0] & 0x80 != 0;
    let opcode = bytes[0] & 0x0F;
    let masked = bytes[1] & 0x80 != 0;
    let (length, mut at) = match bytes[1] & 0x7F {
        126 => (
            u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?) as usize,
            4,
        ),
        127 => {
            let length = u64::from_be_bytes(bytes.get(2..10)?.try_into().ok()?);
            (usize::try_from(length).unwrap_or(usize::MAX), 10)
        }
        short => (short as usize, 2),
    };
    // Every client frame is masked (§5.1); an unmasked one, or one larger
    // than any answer could be, is a fault — decided before waiting for it.
    if !masked || length > MAX_MESSAGE || bytes[0] & 0x70 != 0 {
        return Some((Frame::Bad, bytes.len()));
    }
    let mask: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    at += 4;
    let payload: Vec<u8> = bytes
        .get(at..at + length)?
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ mask[i % 4])
        .collect();
    let frame = match opcode {
        0x0 | 0x1 => Frame::Text { fin, payload },
        0x2 => Frame::Binary,
        0x8 => Frame::Close,
        0x9 => Frame::Ping(payload),
        0xA => Frame::Pong,
        _ => Frame::Bad,
    };
    Some((frame, at + length))
}

/// A server frame (never masked, §5.1).
fn encode(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => out.push(n as u8),
        n if n <= 0xFFFF => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

fn close_frame(code: u16) -> Vec<u8> {
    encode(0x8, &code.to_be_bytes())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Token comparison that does not stop at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_are_written_the_way_a_browser_sends_them() {
        assert_eq!(
            origin_of("https://sign.getvela.app/"),
            "https://sign.getvela.app"
        );
        assert_eq!(
            origin_of("https://Sign.GetVela.app:443/sign.html?x#y"),
            "https://sign.getvela.app"
        );
        assert_eq!(
            origin_of("http://localhost:8140/clearsigning/"),
            "http://localhost:8140"
        );
        assert_eq!(origin_of("not a url"), "");
    }

    #[test]
    fn the_rfc_example_key_is_accepted_with_the_rfc_answer() {
        let head = "GET / HTTP/1.1\r\nHost: 127.0.0.1:1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nOrigin: https://sign.getvela.app";
        let response = accept(head, "https://sign.getvela.app").unwrap_or_else(|| unreachable!());
        assert!(response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
    }
}
