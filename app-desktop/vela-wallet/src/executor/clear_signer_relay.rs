//! Across devices: the Clear Signer relay, from the requester's end (spec 075,
//! `specs/075-clear-signer-channel/contracts/relay.md`).
//!
//! The owner's call — "跨端时，websocket 蓝牙是核心通道" — put a relay between a
//! wallet on this computer and a signer page on the phone in the person's hand.
//! **The relay is blind**: it pairs two sockets in a room and forwards their
//! frames, and everything after the two hellos is sealed. So nothing here trusts
//! it with anything:
//!
//! - the session is the core's (`clear_signer::secure`) — P-256 ECDH, HKDF and
//!   AES-GCM under the `vela-relay/1` label, byte for byte what the page's
//!   `lib/transport/secure.js` runs, and pinned on both sides by
//!   `rust/crates/vela-core/tests/clear-signer/secure-session.json`;
//! - the page refuses a stand-in wallet with `rk`, the fingerprint of this
//!   wallet's key that the pairing link carries;
//! - the person refuses a stand-in page with the **six-digit code**, which both
//!   screens derive and which is confirmed HERE before a single request is
//!   sent. A create matters most: a substituted page would hand the wallet
//!   somebody else's key.
//!
//! This file is the socket, the clock, and the order of the frames. It is the
//! same [`Line`] a loopback socket is, so a create over the relay and a create
//! on this device run the same code above it.

use std::io;
use std::net::TcpStream;
use std::time::Instant;

use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use vela_core::clear_signer::secure::{Handshake, Label, Role, Session, Tail, key_fingerprint};
use vela_core::clear_signer::{self, ws};

use crate::executor::clear_signer::{Channel, Line, POLL, Refusal};
use crate::executor::passkey;

/// What the wallet says it is in its hello — the page shows nothing of it, but
/// the field is part of the wire (relay.md §2.2).
const APP: &str = concat!("vela-desktop/", env!("CARGO_PKG_VERSION"));

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// The randomness one pairing needs, all of it the shell's — the core holds
/// none (`secure.rs`). A test pins it to the shared vectors.
#[derive(Clone, Copy, Debug)]
pub struct Seed {
    /// This pairing's P-256 scalar. New every time: the code and the room are
    /// derived from it, and a reused key would reuse a code.
    pub secret: [u8; 32],
    pub nonce: [u8; 16],
    /// The 128 bits the room id is written from.
    pub room: [u8; 16],
}

impl Seed {
    #[must_use]
    pub fn random() -> Self {
        let bytes = |n: usize| passkey::random(n);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes(32));
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&bytes(16));
        let mut room = [0u8; 16];
        room.copy_from_slice(&bytes(16));
        Self {
            secret,
            nonce,
            room,
        }
    }
}

/// A paired relay session: the socket, the sealed session over it, and the two
/// counters the wire carries (the request id, and the protocol's `n`).
pub struct Relay {
    socket: Socket,
    session: Session,
    /// The request id, so an answer can be matched to its question.
    seq: u64,
    /// `n` on what we send — monotonic per relay.md §2.5.
    sent: u64,
    /// The highest `n` accepted from the page. One that does not increase is
    /// dropped: the seal already refuses a replay, and this refuses a reorder
    /// the relay could have caused.
    seen: u64,
}

/// Pair with a signer page on another device, and hold the line open.
///
/// In order (relay.md §2): the link on screen as a QR and a copyable address,
/// the room, `joined`, the page's hello, ours, the derived code — and then
/// nothing at all until the person has said the two screens agree.
pub fn open(
    page: &str,
    relay: &str,
    seed: Seed,
    channel: &Channel,
    deadline: Instant,
) -> Result<Relay, Refusal> {
    let handshake = Handshake::new(&seed.secret, seed.nonce, Role::Requester).map_err(|error| {
        // A scalar the curve rejects is a ~2⁻³² event; there is nothing for a
        // person to do about it but try again.
        eprintln!("[vela-wallet] clear signer relay: {error}");
        Refusal::Unreachable
    })?;
    let room = clear_signer::relay_room(&seed.room);
    let rk = key_fingerprint(handshake.public_key());
    if !channel.pair(clear_signer::relay_link(page, relay, &room, &rk)) {
        return Err(Refusal::Closed);
    }

    let url = clear_signer::relay_room_url(relay, &room, "requester");
    let (mut socket, _response) = tungstenite::connect(&url).map_err(|error| {
        eprintln!("[vela-wallet] clear signer relay: {url} did not answer: {error}");
        Refusal::Unreachable
    })?;
    set_poll_timeout(&mut socket);

    // `joined`, then the page's hello. Nothing is sent before `joined`: a
    // frame with no peer in the room is dropped, never buffered (§1).
    let mut joined = false;
    let peer_hello = loop {
        let text = match recv(&mut socket, channel, deadline)? {
            Message::Text(text) => text,
            Message::Binary(_) => continue,
            _ => continue,
        };
        let frame: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        match frame.get("relay").and_then(Value::as_str) {
            // The page arrived, or came back after a drop.
            Some("joined") => {
                joined = true;
                continue;
            }
            // It left before saying anything; the room waits for it.
            Some(_) => {
                joined = false;
                continue;
            }
            None if joined => break text,
            // A frame before `joined` is not the page's.
            None => continue,
        }
    };

    send(&mut socket, Message::text(handshake.hello(Some(APP))))?;
    // The page checks our key against `rk`; we have no fingerprint to check
    // it against — the code the person reads is what does that job here.
    let session = handshake
        .complete(&peer_hello, Label::Relay, None)
        .map_err(|error| {
            eprintln!("[vela-wallet] clear signer relay: bad hello: {error}");
            Refusal::Mismatch
        })?;

    // The one gate before anything leaves this wallet (§2.4).
    channel.show_code(session.code());
    while !channel.code_confirmed() {
        if channel.stopped() {
            return Err(Refusal::Closed);
        }
        if Instant::now() >= deadline {
            return Err(Refusal::TimedOut);
        }
        // The relay's own ping keeps the room alive while the person reads;
        // a frame arriving here would be one nobody asked for.
        drain(&mut socket)?;
        std::thread::sleep(POLL);
    }

    Ok(Relay {
        socket,
        session,
        seq: 0,
        sent: 0,
        seen: 0,
    })
}

impl Line for Relay {
    fn next_id(&mut self) -> String {
        self.seq += 1;
        format!("r{}", self.seq)
    }

    fn ask(
        &mut self,
        id: &str,
        request: &Value,
        channel: &Channel,
        deadline: Instant,
    ) -> Result<Value, Refusal> {
        self.sent += 1;
        let intent = json!({
            "v": 1,
            "t": "intent",
            "n": self.sent,
            "id": id,
            "intent": request.get("intent").cloned().unwrap_or(Value::Null),
            "context": request.get("context").cloned().unwrap_or(Value::Null),
        });
        let sealed = self
            .session
            .seal(intent.to_string().as_bytes(), Tail::Counter);
        send(&mut self.socket, Message::binary(sealed))?;
        loop {
            let sealed = match recv(&mut self.socket, channel, deadline)? {
                Message::Binary(bytes) => bytes,
                // The relay's own frames are text and have a `relay` key; the
                // page sends nothing else in the clear after the handshake.
                Message::Text(text) => {
                    if relay_said_left(&text) {
                        return Err(Refusal::Closed);
                    }
                    continue;
                }
                _ => continue,
            };
            let Ok(plain) = self.session.open(&sealed, Tail::Counter) else {
                // A message that will not open is the relay's doing or a
                // replay; neither is this page answering.
                continue;
            };
            let text = String::from_utf8_lossy(&plain).into_owned();
            let Ok(message) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            let n = message.get("n").and_then(Value::as_u64).unwrap_or_default();
            if n <= self.seen {
                continue;
            }
            self.seen = n;
            if message.get("t").and_then(Value::as_str) == Some("bye") {
                return Err(Refusal::Closed);
            }
            match ws::parse_message(&text) {
                ws::Message::Result { id: got, result } if got == id => return Ok(result),
                ws::Message::Error { id: got, error } if got == id => {
                    return Err(Refusal::of(&error));
                }
                _ => {}
            }
        }
    }

    fn end(&mut self) {
        self.sent += 1;
        let bye = json!({ "v": 1, "t": "bye", "n": self.sent, "reason": "done" });
        let sealed = self.session.seal(bye.to_string().as_bytes(), Tail::Counter);
        let _ = self.socket.send(Message::binary(sealed));
        let _ = self.socket.flush();
        let _ = self.socket.close(None);
        let _ = self.socket.flush();
    }
}

// ---------------------------------------------------------------------------
// The socket
// ---------------------------------------------------------------------------

/// Read with a short timeout, so every wait can look at the clock and at the
/// person's "Cancel" between frames.
fn set_poll_timeout(socket: &mut Socket) {
    let timeout = Some(POLL);
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(timeout);
        }
        MaybeTlsStream::Rustls(stream) => {
            let _ = stream.sock.set_read_timeout(timeout);
        }
        _ => {}
    }
}

fn send(socket: &mut Socket, message: Message) -> Result<(), Refusal> {
    // Written out rather than chained: tungstenite's own error is 136 bytes,
    // and a closure that carried it would put that in every `Result` on this
    // path for a value nobody outside this function ever reads.
    let mut trouble = socket.send(message).err();
    if trouble.is_none() {
        trouble = socket.flush().err();
    }
    match trouble {
        None => Ok(()),
        Some(error) => {
            eprintln!("[vela-wallet] clear signer relay: could not write: {error}");
            Err(Refusal::Unreachable)
        }
    }
}

/// The next message from the room, waiting on the clock and the screen.
fn recv(socket: &mut Socket, channel: &Channel, deadline: Instant) -> Result<Message, Refusal> {
    loop {
        if channel.stopped() {
            return Err(Refusal::Closed);
        }
        if Instant::now() >= deadline {
            return Err(Refusal::TimedOut);
        }
        match socket.read() {
            Ok(Message::Close(frame)) => return Err(closed_by(frame.as_ref())),
            Ok(message) => return Ok(message),
            Err(tungstenite::Error::Io(error)) if would_block(&error) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Err(Refusal::Unreachable);
            }
            Err(error) => {
                eprintln!("[vela-wallet] clear signer relay: {error}");
                return Err(Refusal::Unreachable);
            }
        }
    }
}

/// Take whatever is waiting, once, without blocking — used while the person
/// reads the code, so a relay that hangs up is noticed then rather than
/// minutes later.
fn drain(socket: &mut Socket) -> Result<(), Refusal> {
    match socket.read() {
        Ok(Message::Close(frame)) => Err(closed_by(frame.as_ref())),
        Ok(Message::Text(text)) if relay_said_left(&text) => Err(Refusal::Closed),
        Ok(_) => Ok(()),
        Err(tungstenite::Error::Io(error)) if would_block(&error) => Ok(()),
        Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
            Err(Refusal::Unreachable)
        }
        Err(error) => {
            eprintln!("[vela-wallet] clear signer relay: {error}");
            Err(Refusal::Unreachable)
        }
    }
}

fn would_block(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}

/// Why the relay hung up, in a sentence the sheet can say (relay.md §1's close
/// codes). 4408 is the room's ten minutes or an idle end — a wait that ran out,
/// which is what the person actually saw; everything else, including 4409
/// (somebody already in our role) and a peer that just went, reads as a relay
/// that could not carry this.
fn closed_by(frame: Option<&tungstenite::protocol::CloseFrame<'_>>) -> Refusal {
    let code = frame.map(|frame| u16::from(frame.code));
    match code {
        Some(4408) => Refusal::TimedOut,
        _ => {
            if let Some(frame) = frame {
                eprintln!(
                    "[vela-wallet] clear signer relay: closed {} {}",
                    u16::from(frame.code),
                    frame.reason
                );
            }
            Refusal::Unreachable
        }
    }
}

fn relay_said_left(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|frame| {
            frame
                .get("relay")
                .and_then(Value::as_str)
                .map(|what| what == "left")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{Read as _, Write as _};
    use std::net::{Ipv4Addr, TcpListener};
    use std::sync::Arc;
    use std::time::Duration;

    use vela_core::primitives::{from_hex, to_hex};

    use crate::executor::clear_signer::Place;
    use crate::executor::clear_signer::tests::{keys, page_assertion, signing_key};

    /// The vectors both ends are pinned to — the page's `secure.js` reads the
    /// same file (`samples/secure-vectors.mjs`).
    const VECTORS: &str =
        include_str!("../../../../rust/crates/vela-core/tests/clear-signer/secure-session.json");

    fn vectors() -> Value {
        serde_json::from_str(VECTORS).unwrap_or_else(|e| unreachable!("{e}"))
    }

    fn bytes<const N: usize>(hex: &str) -> [u8; N] {
        let mut out = [0u8; N];
        out.copy_from_slice(&from_hex(hex).unwrap_or_else(|e| unreachable!("{e}")));
        out
    }

    /// The relay's first case, as a seed this requester can be run with.
    fn seeded(case: &Value) -> Seed {
        Seed {
            secret: bytes::<32>(case["requester"]["secretHex"].as_str().unwrap_or_default()),
            nonce: bytes::<16>(case["requester"]["nonceHex"].as_str().unwrap_or_default()),
            room: [0x5a; 16],
        }
    }

    /// A stand-in relay AND the page behind it, on one loopback socket: it
    /// speaks relay.md §1's frames (`joined`, forwarding) and then the page's
    /// half of §2. Everything the wallet sends after the hellos arrives here
    /// sealed, which is what lets a test assert the relay saw no plaintext.
    struct FakeRelay {
        port: u16,
        /// Every frame the "relay" forwarded, in order — text or binary.
        seen: Arc<std::sync::Mutex<Vec<Message>>>,
        page: std::sync::mpsc::Receiver<PageEnd>,
    }

    /// The page's end of the room, handed to the test once a wallet connects.
    struct PageEnd {
        socket: tungstenite::WebSocket<TcpStream>,
        seen: Arc<std::sync::Mutex<Vec<Message>>>,
    }

    impl FakeRelay {
        fn start() -> Self {
            let listener =
                TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap_or_else(|e| unreachable!("{e}"));
            let port = listener
                .local_addr()
                .map(|address| address.port())
                .unwrap_or_default();
            let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
            let (ready, page) = std::sync::mpsc::channel();
            let tapped = Arc::clone(&seen);
            std::thread::spawn(move || {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
                let socket = tungstenite::accept(stream)
                    .unwrap_or_else(|e| unreachable!("the wallet's upgrade: {e}"));
                let _ = ready.send(PageEnd {
                    socket,
                    seen: tapped,
                });
            });
            Self { port, seen, page }
        }

        fn url(&self) -> String {
            format!("ws://127.0.0.1:{}", self.port)
        }

        /// The page's end, once the wallet has joined the room.
        fn page(&self) -> PageEnd {
            self.page
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|e| unreachable!("the wallet never connected: {e}"))
        }
    }

    impl PageEnd {
        fn joined(&mut self) {
            let _ = self
                .socket
                .send(Message::text(r#"{"v":1,"relay":"joined"}"#));
            let _ = self.socket.flush();
        }

        fn say(&mut self, message: Message) {
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(message.clone());
            }
            let _ = self.socket.send(message);
            let _ = self.socket.flush();
        }

        fn next(&mut self) -> Message {
            loop {
                let message = self
                    .socket
                    .read()
                    .unwrap_or_else(|e| unreachable!("the wallet stopped talking: {e}"));
                if matches!(message, Message::Ping(_) | Message::Pong(_)) {
                    continue;
                }
                if let Ok(mut seen) = self.seen.lock() {
                    seen.push(message.clone());
                }
                return message;
            }
        }
    }

    /// The pairing, byte for byte against the shared vectors: the link's `rk`,
    /// the six digits, and every message sealed exactly as the page's
    /// `secure.js` seals it. The relay never sees a plaintext request.
    #[test]
    fn the_requester_pairs_and_seals_as_the_vectors_say() {
        let cases = vectors();
        let case = cases["cases"]
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["name"] == "relay-a"))
            .unwrap_or_else(|| unreachable!("the relay-a vector"));

        let relay = FakeRelay::start();
        let (channel, _changed) = Channel::new();
        let seed = seeded(case);
        let expected_code = case["code"].as_str().unwrap_or_default().to_owned();
        let expected_rk = case["rk"].as_str().unwrap_or_default().to_owned();

        // The person, answering the two questions the pairing asks.
        let watcher = {
            let channel = Arc::clone(&channel);
            let code = expected_code.clone();
            let rk = expected_rk.clone();
            std::thread::spawn(move || {
                let mut link = String::new();
                for _ in 0..1200 {
                    if let Some(pairing) = channel.pairing() {
                        if link.is_empty() {
                            link = pairing.link.clone();
                        }
                        if pairing.code.as_deref() == Some(code.as_str()) {
                            channel.confirm_code();
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                assert!(
                    link.contains(&format!("rk={rk}")),
                    "the link must carry this wallet's fingerprint: {link}"
                );
                assert!(link.contains("ch=relay"), "{link}");
                link
            })
        };

        let url = relay.url();
        let paired = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || {
                open(
                    "https://sign.getvela.app/",
                    &url,
                    seed,
                    &channel,
                    Instant::now() + Duration::from_secs(30),
                )
            })
        };

        let mut page = relay.page();
        page.joined();
        page.say(Message::text(
            case["signer"]["hello"].as_str().unwrap_or_default(),
        ));
        // The wallet's own hello: plaintext, and the only plaintext it sends.
        let hello = page.next();
        let Message::Text(hello) = hello else {
            unreachable!("the wallet's hello is a text frame");
        };
        let ours: Value = serde_json::from_str(&hello).unwrap_or_else(|e| unreachable!("{e}"));
        let theirs: Value =
            serde_json::from_str(case["requester"]["hello"].as_str().unwrap_or_default())
                .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(ours["pk"], theirs["pk"], "the vector's public key");
        assert_eq!(ours["nonce"], theirs["nonce"]);
        assert_eq!(ours["role"], "requester");

        let mut line = paired
            .join()
            .unwrap_or_else(|_| unreachable!("the pairing panicked"))
            .unwrap_or_else(|refusal| unreachable!("the pairing failed: {refusal:?}"));
        let _ = watcher.join();

        // The page's side of the session, from the vector's own secret.
        let mut signer = Handshake::new(
            &bytes::<32>(case["signer"]["secretHex"].as_str().unwrap_or_default()),
            bytes::<16>(case["signer"]["nonceHex"].as_str().unwrap_or_default()),
            Role::Signer,
        )
        .unwrap_or_else(|e| unreachable!("{e}"))
        .complete(&hello, Label::Relay, Some(&expected_rk))
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(signer.code(), expected_code, "both screens' six digits");

        // One request, sealed both ways.
        let id = line.next_id();
        let answering = std::thread::spawn(move || {
            let sealed = match page.next() {
                Message::Binary(bytes) => bytes,
                other => unreachable!("a request must be a binary frame: {other:?}"),
            };
            let plain = signer
                .open(&sealed, Tail::Counter)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            let intent: Value =
                serde_json::from_str(&String::from_utf8_lossy(&plain)).unwrap_or_default();
            assert_eq!(intent["t"], "intent");
            assert_eq!(intent["intent"]["method"], "vela_signIn");
            let answer = json!({
                "v": 1, "t": "result", "n": 2, "id": intent["id"],
                "assertion": page_assertion(&signing_key(7), &[0x11, 0x22, 0x33], b"vela-signin-1-0123456789abcdef"),
                "origin": "https://sign.getvela.app",
            });
            page.say(Message::binary(
                signer.seal(answer.to_string().as_bytes(), Tail::Counter),
            ));
            page
        });
        let request = json!({
            "intent": { "method": "vela_signIn", "params": [{}], "origin": "" },
            "context": { "walletName": "Everyday" },
        });
        let answer = line
            .ask(
                &id,
                &request,
                &channel,
                Instant::now() + Duration::from_secs(30),
            )
            .unwrap_or_else(|refusal| unreachable!("{refusal:?}"));
        assert!(answer.get("assertion").is_some(), "{answer}");
        let mut page = answering
            .join()
            .unwrap_or_else(|_| unreachable!("the page panicked"));
        line.end();
        // Everything after the two hellos was ciphertext on the wire.
        let seen = relay.seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let plaintext: Vec<&Message> = seen
            .iter()
            .filter(|message| matches!(message, Message::Text(_)))
            .collect();
        assert_eq!(
            plaintext.len(),
            2,
            "only the two hellos travel in the clear: {plaintext:?}"
        );
        let _ = page.socket.close(None);
        let _ = keys();
    }

    /// The relay that will not answer at all: the sheet says so, and the
    /// request stays open to be signed another way.
    #[test]
    fn a_relay_that_cannot_be_reached_says_so() {
        // A port nothing is listening on — bound and dropped, so it is free.
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|listener| listener.local_addr())
            .map(|address| address.port())
            .unwrap_or(1);
        let (channel, _changed) = Channel::new();
        let refusal = open(
            "https://sign.getvela.app/",
            &format!("ws://127.0.0.1:{port}"),
            Seed::random(),
            &channel,
            Instant::now() + Duration::from_secs(5),
        )
        .err()
        .unwrap_or_else(|| unreachable!("a dead relay paired"));
        assert_eq!(refusal, Refusal::Unreachable);
        assert_eq!(
            refusal.key(),
            "componentsUi.signing.clearSignerRelayDown",
            "the sentence the sheet says"
        );
    }

    /// Nothing leaves the wallet until the person says the two screens show
    /// the same six digits (relay.md §2.4) — the check that stops a stand-in
    /// page from being handed a create.
    #[test]
    fn no_request_is_sent_before_the_person_confirms_the_code() {
        let cases = vectors();
        let case = cases["cases"]
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["name"] == "relay-a"))
            .unwrap_or_else(|| unreachable!("the relay-a vector"));
        let relay = FakeRelay::start();
        let (channel, _changed) = Channel::new();
        let seed = seeded(case);
        let url = relay.url();
        let paired = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || {
                open(
                    "https://sign.getvela.app/",
                    &url,
                    seed,
                    &channel,
                    Instant::now() + Duration::from_secs(20),
                )
            })
        };
        let mut page = relay.page();
        page.joined();
        page.say(Message::text(
            case["signer"]["hello"].as_str().unwrap_or_default(),
        ));
        let _hello = page.next();

        // The code is on screen and the pairing is NOT finished: the line does
        // not exist yet, so nothing can be asked over it.
        let mut showed = None;
        for _ in 0..1200 {
            if let Some(pairing) = channel.pairing()
                && pairing.code.is_some()
            {
                showed = pairing.code.clone();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(showed.as_deref(), case["code"].as_str());
        assert!(!paired.is_finished(), "paired before the person confirmed");

        channel.confirm_code();
        assert!(
            paired
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_ok()
        );
        let _ = page.socket.close(None);
        let _ = Place::OtherDevice;
    }

    /// Reading the vectors' own hex must not drift: a seed that is not the
    /// vector's would derive a different code and prove nothing.
    #[test]
    fn the_vector_seed_reproduces_the_vectors_own_fingerprint() {
        let cases = vectors();
        for case in cases["cases"].as_array().unwrap_or(&vec![]) {
            if case["label"] != "vela-relay/1" {
                continue;
            }
            let seed = seeded(case);
            let handshake = Handshake::new(&seed.secret, seed.nonce, Role::Requester)
                .unwrap_or_else(|e| unreachable!("{e:?}"));
            assert_eq!(
                to_hex(handshake.public_key(), false),
                case["requester"]["publicKeyHex"]
                    .as_str()
                    .unwrap_or_default(),
                "{}",
                case["name"]
            );
            assert_eq!(
                key_fingerprint(handshake.public_key()),
                case["rk"].as_str().unwrap_or_default(),
                "{}",
                case["name"]
            );
        }
    }

    /// A read that timed out is not a broken socket — without this the first
    /// poll would end every pairing.
    #[test]
    fn a_polling_timeout_is_not_a_failure() {
        for kind in [
            io::ErrorKind::WouldBlock,
            io::ErrorKind::TimedOut,
            io::ErrorKind::Interrupted,
        ] {
            assert!(would_block(&io::Error::new(kind, "poll")));
        }
        assert!(!would_block(&io::Error::new(
            io::ErrorKind::ConnectionReset,
            "gone"
        )));
    }

    /// The relay's own frames are the two it may send; a `left` while a
    /// request is out is the page going away.
    #[test]
    fn the_relays_own_frames_are_read_as_its_own() {
        assert!(relay_said_left(r#"{"v":1,"relay":"left"}"#));
        assert!(!relay_said_left(r#"{"v":1,"relay":"joined"}"#));
        assert!(!relay_said_left(r#"{"v":1,"t":"result"}"#));
        assert!(!relay_said_left("not json"));
    }

    /// A stray helper the tests above lean on: reading a whole HTTP head.
    #[test]
    fn the_fake_relay_speaks_websocket() {
        let relay = FakeRelay::start();
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let head = format!(
            "GET /v1/rooms/x?role=requester HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
             Upgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            relay.port
        );
        let _ = stream.write_all(head.as_bytes());
        let mut buffer = [0u8; 256];
        let read = stream.read(&mut buffer).unwrap_or_default();
        let response = String::from_utf8_lossy(&buffer[..read]).into_owned();
        assert!(response.starts_with("HTTP/1.1 101"), "{response}");
    }
}
