//! The PeerConnect transport behind the multisig machines' `peer_*`
//! operations ([`Rooms`]): rooms (Noise end to end; the relay reads
//! nothing), and the relay's token. The web's `shell/peer.ts` and
//! `external/relay.ts`, in-process — and nothing of WalletPair: what is sent
//! and what an arrival means are the core's (`multisig_outside`).
//!
//! A room is known to the core by a handle this transport hands out
//! (`room-N`). What the connection says of itself goes back to the machine
//! that opened it as its own events ([`PeerEvent`]): its state (offline /
//! waiting / connected / closed), `paired` (the record that reopens it now)
//! and `relay` (where getting onto the relay stands).
//!
//! The protocol is PeerConnect's own sans-I/O [`Peer`] (`rust/vendor/
//! peerconnect`, the same Rust the web runs as wasm); this file supplies the
//! WebSocket, the timers and the threads, as the JS SDK's `Connection` does
//! for the browser. Each open room has one thread of its own that owns the
//! socket and the machine; [`Rooms::receive`] blocks on the room's inbox.
//!
//! The relay token: the public relay admits a connection only with a token,
//! asked of Vela's site under this device's ID (else PeerConnect's public
//! trial) and kept until it expires. Every decision about it — which relay
//! asks for one, whether the kept one is still good, what an endpoint
//! answered, which token a refusal forgets, how long to wait and when to
//! stop, where the way onto the relay stands — is the core's
//! (`multisig_outside::relay`, and `relay.onState` through the rules door).
//! This file keeps only the I/O: the socket, the waits, the fetches and the
//! device's store, under the web's keys.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, Hasher};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use peerconnect::{CloseReason, Event, Invite, Output, Peer, Room, State, wire};
use serde_json::{Value, json};
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tungstenite::stream::MaybeTlsStream;
use vela_core::app::multisig_outside::relay::{
    self as rules, Before, KeptToken, Refusals, RelayStatus, RelayTrouble, TokenSource,
};

use super::shell::{Arrival, Opened, Rooms};

/// What a connection tells the machine that opened it.
#[derive(Clone, Debug, PartialEq)]
pub enum PeerEvent {
    /// `offline` / `waiting` / `connected` / `closed`.
    State { room: String, state: String },
    /// Pairing is complete: `pairing` is the record that reopens the room now.
    Paired { room: String, pairing: String },
    /// Where this side's way onto the relay stands.
    Relay { room: String, status: RelayStatus },
}

/// Where a room's events go.
pub type Tell = Arc<dyn Fn(PeerEvent) + Send + Sync>;

/// The relay new rooms go through, when the device names one (the web's key).
pub const RELAY_KEY: &str = "vela.peerconnectRelay";
/// Where tokens are asked for, when the device names a place (a local site).
pub const ENDPOINT_KEY: &str = "vela.peerconnectTokenEndpoint";
/// This device's ID at Vela's site.
pub const DEVICE_KEY: &str = "vela.peerconnectDevice";
/// The token kept for the public relay ([`KeptToken`]).
pub const TOKEN_KEY: &str = "vela.peerconnectToken.v1";

/// How long a room's thread reads its socket before it looks at what the
/// app asked of it (a send, a leave): the latency of a send, at most.
const TICK: Duration = Duration::from_millis(25);
/// The socket's opening handshakes (TLS, WebSocket), at most.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// One write, at most: a relay that stops reading must not hold the thread.
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);
/// One token endpoint's answer, at most.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------------------
// The I/O seam
// ---------------------------------------------------------------------------

/// A WebSocket message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Binary(Vec<u8>),
    Text(String),
}

/// What one read of a socket found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    Message(Message),
    /// Nothing within the wait.
    Nothing,
    /// The socket closed or failed, with the close code when one came.
    Closed(Option<u16>),
}

/// One open WebSocket, owned by its room's thread.
pub trait Socket: Send {
    /// Send a message; a failure shows as the next read's close.
    fn send(&mut self, message: Message);
    /// The next message, waiting `wait` at most.
    fn read(&mut self, wait: Duration) -> Read;
    /// Close (code 1000) and forget it.
    fn close(&mut self);
}

/// The network and the device's store, as the transport uses them: the
/// app's ([`LiveNet`]), or a test's.
pub trait Net: Send + Sync + 'static {
    /// A WebSocket to `url` (`ws://` or `wss://`), its handshakes done, or
    /// why not. Blocks; runs on a thread of its own.
    fn connect(&self, url: &str) -> Result<Box<dyn Socket>, String>;
    /// POST to `url`, with `body` as JSON when there is one: the status and
    /// the body as they came, or no status when nothing answered.
    fn post(&self, url: &str, body: Option<&Value>) -> (Option<u16>, String);
    /// A stored value (the web's `localStorage`, by the same key).
    fn load(&self, key: &str) -> Option<Value>;
    /// Store a value, or remove it (`None`).
    fn store(&self, key: &str, value: Option<Value>);
}

/// The app's network — the app's proxy routes, as the cable tunnel and the
/// AI agent's bridge dial — and its store.
pub struct LiveNet;

impl Net for LiveNet {
    fn connect(&self, url: &str) -> Result<Box<dyn Socket>, String> {
        let socket = WsSocket::open(url, |host, port, secure| {
            let scheme = if secure { "https" } else { "http" };
            crate::ctap::cable::dial(&format!("{scheme}://{host}:{port}/"), host, port, |line| {
                crate::diag::vlog!("peer", "{line}");
            })
            .map_err(|error| error.to_string())
        })?;
        Ok(Box::new(socket))
    }

    fn post(&self, url: &str, body: Option<&Value>) -> (Option<u16>, String) {
        let body = body.map(Value::to_string);
        let answer = crate::executor::proxy::with_routes(url, TOKEN_TIMEOUT, |agent| {
            let request = agent
                .post(url)
                .config()
                .http_status_as_error(false)
                .build()
                .header("accept", "application/json");
            let mut response = match &body {
                Some(body) => request
                    .header("content-type", "application/json")
                    .send(body.as_str())?,
                None => request.send_empty()?,
            };
            let status = response.status().as_u16();
            let text = response.body_mut().read_to_string().unwrap_or_default();
            Ok((status, text))
        });
        match answer {
            Ok((status, text)) => (Some(status), text),
            // No answer: said as that, by the core.
            Err(_) => (None, String::new()),
        }
    }

    fn load(&self, key: &str) -> Option<Value> {
        crate::executor::storage::read_value(key).ok().flatten()
    }

    fn store(&self, key: &str, value: Option<Value>) {
        // A store that cannot be written asks again next time (the web's
        // private mode).
        let _ = match value {
            Some(value) => crate::executor::storage::write_value(key, value),
            None => crate::executor::storage::remove_value(key),
        };
    }
}

/// A blocking tungstenite WebSocket over a socket `dial` connected.
pub struct WsSocket {
    ws: tungstenite::WebSocket<MaybeTlsStream<TcpStream>>,
}

impl WsSocket {
    /// Open `url` over the TCP stream `dial(host, port, secure)` gives: TLS
    /// for `wss://`, then the WebSocket handshake. The URL carries the
    /// token, so no error here repeats it.
    pub fn open(
        url: &str,
        dial: impl FnOnce(&str, u16, bool) -> Result<TcpStream, String>,
    ) -> Result<Self, String> {
        let parsed = url::Url::parse(url).map_err(|error| format!("relay URL: {error}"))?;
        let secure = match parsed.scheme() {
            "wss" => true,
            "ws" => false,
            other => return Err(format!("not a WebSocket URL ({other}:)")),
        };
        let host = match parsed.host() {
            Some(url::Host::Domain(name)) => name.to_owned(),
            Some(url::Host::Ipv4(address)) => address.to_string(),
            Some(url::Host::Ipv6(address)) => address.to_string(),
            None => return Err("relay URL without a host".to_owned()),
        };
        let port = parsed
            .port_or_known_default()
            .unwrap_or(if secure { 443 } else { 80 });
        let stream = dial(&host, port, secure)?;
        let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
        let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
        let _ = stream.set_nodelay(true);
        let config = WebSocketConfig {
            max_message_size: Some(wire::MAX_FRAME_LEN),
            max_frame_size: Some(wire::MAX_FRAME_LEN),
            ..WebSocketConfig::default()
        };
        let (ws, _response) = tungstenite::client_tls_with_config(url, stream, Some(config), None)
            .map_err(|error| match error {
                tungstenite::HandshakeError::Failure(error) => describe(&error),
                tungstenite::HandshakeError::Interrupted(_) => "handshake timed out".to_owned(),
            })?;
        Ok(Self { ws })
    }

    fn tcp(&self) -> Option<&TcpStream> {
        match self.ws.get_ref() {
            MaybeTlsStream::Plain(socket) => Some(socket),
            MaybeTlsStream::Rustls(tls) => Some(&tls.sock),
            _ => None,
        }
    }
}

/// A WebSocket failure in words that never carry the URL (it holds the token).
fn describe(error: &tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Http(response) => format!("HTTP {}", response.status().as_u16()),
        tungstenite::Error::Url(_) => "relay URL refused".to_owned(),
        other => other.to_string(),
    }
}

impl Socket for WsSocket {
    fn send(&mut self, message: Message) {
        let message = match message {
            Message::Binary(bytes) => tungstenite::Message::Binary(bytes),
            Message::Text(text) => tungstenite::Message::Text(text),
        };
        let _ = self.ws.send(message);
    }

    fn read(&mut self, wait: Duration) -> Read {
        if let Some(tcp) = self.tcp() {
            let _ = tcp.set_read_timeout(Some(wait.max(Duration::from_millis(1))));
        }
        match self.ws.read() {
            Ok(tungstenite::Message::Binary(bytes)) => Read::Message(Message::Binary(bytes)),
            Ok(tungstenite::Message::Text(text)) => Read::Message(Message::Text(text)),
            Ok(tungstenite::Message::Close(frame)) => {
                Read::Closed(frame.map(|frame| u16::from(frame.code)))
            }
            // A ping: its pong is queued; it goes out now.
            Ok(_) => {
                let _ = self.ws.flush();
                Read::Nothing
            }
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Read::Nothing
            }
            Err(_) => Read::Closed(None),
        }
    }

    fn close(&mut self) {
        let _ = self.ws.close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        }));
        let _ = self.ws.flush();
    }
}

// ---------------------------------------------------------------------------
// The transport
// ---------------------------------------------------------------------------

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The transport for one machine; `tell` hands its events to that machine.
pub struct PeerRooms {
    tell: Tell,
    net: Arc<dyn Net>,
    rooms: Mutex<HashMap<String, Arc<Open>>>,
    next: AtomicU64,
}

/// One open room, as the app side holds it.
struct Open {
    inbox: Mutex<Inbox>,
    arrived: Condvar,
    inputs: Sender<Input>,
    watch: Arc<Watch>,
}

#[derive(Default)]
struct Inbox {
    /// Messages that arrived with nobody waiting.
    queue: VecDeque<Value>,
    /// Why the room closed, once it has.
    closed: Option<String>,
}

impl Open {
    /// A message: to the one waiting, else queued for the next receive.
    fn deliver(&self, message: Value) {
        lock(&self.inbox).queue.push_back(message);
        self.arrived.notify_all();
    }

    /// The room closed (`why`): to the one waiting, else to the next receive.
    fn close_with(&self, why: &str) {
        lock(&self.inbox).closed = Some(why.to_owned());
        self.arrived.notify_all();
    }

    fn receive(&self) -> Arrival {
        let mut inbox = lock(&self.inbox);
        loop {
            if let Some(message) = inbox.queue.pop_front() {
                return Arrival::Message(message);
            }
            if let Some(why) = &inbox.closed {
                return Arrival::Closed(why.clone());
            }
            inbox = self
                .arrived
                .wait(inbox)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl PeerRooms {
    /// Over the app's network and store.
    pub fn new(tell: Tell) -> Arc<Self> {
        Self::with_net(tell, Arc::new(LiveNet))
    }

    /// Over `net` (a test's).
    pub fn with_net(tell: Tell, net: Arc<dyn Net>) -> Arc<Self> {
        Arc::new(Self {
            tell,
            net,
            rooms: Mutex::new(HashMap::new()),
            next: AtomicU64::new(0),
        })
    }

    fn find(&self, room: &str) -> Option<Arc<Open>> {
        lock(&self.rooms).get(room).cloned()
    }

    /// Try the relay again now: a wait cut short, a stop undone (the
    /// person's Retry), and a connection waiting out its backoff tries at
    /// once.
    pub fn retry(&self, room: &str) {
        if let Some(open) = self.find(room) {
            open.watch.retry();
            let _ = open.inputs.send(Input::Now);
        }
    }

    /// Leave every room (the screen went away): pairings are kept.
    pub fn leave_all(&self) {
        let rooms: Vec<Arc<Open>> = lock(&self.rooms).drain().map(|(_, open)| open).collect();
        for open in rooms {
            let _ = open.inputs.send(Input::Leave);
            open.close_with("left");
        }
    }
}

impl Drop for PeerRooms {
    fn drop(&mut self) {
        self.leave_all();
    }
}

impl Rooms for PeerRooms {
    fn open(
        &self,
        relay: Option<&str>,
        invite: Option<&str>,
        pairing: Option<&str>,
    ) -> Result<Opened, String> {
        let room = match (pairing, invite) {
            (Some(pairing), _) => pairing.parse::<Room>().map_err(|e| e.to_string())?,
            (None, Some(invite)) => {
                Room::join(&invite.parse::<Invite>().map_err(|e| e.to_string())?)
            }
            (None, None) => {
                let relay = relay.map_or_else(|| relay_url(&*self.net), str::to_owned);
                Room::create(&relay).map_err(|e| e.to_string())?
            }
        };
        let fresh = pairing.is_none() && invite.is_none();
        let handle = format!("room-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let opened = Opened {
            room: handle.clone(),
            invite: if fresh {
                room.invite().map(|invite| invite.to_string())
            } else {
                None
            },
            pairing: Some(room.to_string()),
        };

        let token = rules::needs_token(room.relay());
        let watch = Arc::new(Watch::new(
            handle.clone(),
            room.relay().to_owned(),
            self.tell.clone(),
        ));
        let (inputs, received) = mpsc::channel();
        let open = Arc::new(Open {
            inbox: Mutex::new(Inbox::default()),
            arrived: Condvar::new(),
            inputs: inputs.clone(),
            watch: watch.clone(),
        });
        let clock = Clock::new();
        let peer = if token {
            // Offline until the first token: the relay would refuse us.
            Peer::awaiting_token(room, clock.now())
        } else {
            Peer::new(room, clock.now())
        };
        let driver = Driver {
            handle: handle.clone(),
            peer,
            clock,
            net: self.net.clone(),
            inputs: received,
            back: inputs,
            socket: None,
            attempt: 0,
            connecting: false,
            open: open.clone(),
            watch,
            tell: self.tell.clone(),
            token,
            state: "offline",
            unreachable_at: None,
        };
        lock(&self.rooms).insert(handle.clone(), open);
        if let Err(error) = thread::Builder::new()
            .name(format!("peerconnect {handle}"))
            .spawn(move || driver.run())
        {
            lock(&self.rooms).remove(&handle);
            return Err(error.to_string());
        }
        Ok(opened)
    }

    fn send(&self, room: &str, message: &Value) {
        let Some(open) = self.find(room) else { return };
        if lock(&open.inbox).closed.is_some() {
            return;
        }
        let _ = open.inputs.send(Input::Send(write_message(message)));
    }

    fn receive(&self, room: &str) -> Arrival {
        match self.find(room) {
            Some(open) => open.receive(),
            None => Arrival::Closed("closed".to_owned()),
        }
    }

    fn close(&self, room: &str, end: bool) {
        // `walletpair_cancel` is the core's message, sent before this.
        let Some(open) = lock(&self.rooms).remove(room) else {
            return;
        };
        let _ = open
            .inputs
            .send(if end { Input::End } else { Input::Leave });
        open.close_with(if end { "closed" } else { "left" });
    }
}

/// A message as it arrived: JSON when it reads as JSON, else its text.
fn read_message(bytes: &[u8]) -> Value {
    let text = String::from_utf8_lossy(bytes);
    serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.into_owned()))
}

/// A message as it is sent: a string as its text, anything else as JSON.
fn write_message(message: &Value) -> Vec<u8> {
    match message {
        Value::String(text) => text.as_bytes().to_vec(),
        other => other.to_string().into_bytes(),
    }
}

/// The relay new rooms go through: the device's, else the public one.
fn relay_url(net: &dyn Net) -> String {
    stored_text(net, RELAY_KEY).unwrap_or_else(|| rules::DEFAULT_RELAY.to_owned())
}

/// Where tokens are asked for: the device's place, else Vela's site.
fn token_endpoint(net: &dyn Net) -> String {
    stored_text(net, ENDPOINT_KEY).unwrap_or_else(|| rules::TOKEN_ENDPOINT.to_owned())
}

fn stored_text(net: &dyn Net, key: &str) -> Option<String> {
    net.load(key)
        .and_then(|value| value.as_str().map(str::trim).map(str::to_owned))
        .filter(|text| !text.is_empty())
}

fn now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64())
}

fn epoch_ms() -> f64 {
    now_s() * 1000.0
}

// ---------------------------------------------------------------------------
// One room's thread
// ---------------------------------------------------------------------------

/// What a room's thread is asked, or told.
enum Input {
    Send(Vec<u8>),
    Leave,
    End,
    Token(String),
    /// Try the relay now, if waiting out a backoff.
    Now,
    /// A socket asked for (`attempt`) opened, or did not.
    Opened {
        attempt: u64,
        socket: Result<Box<dyn Socket>, String>,
    },
}

/// The machine's clock: milliseconds since the room opened, moved ahead by
/// a wait cut short.
struct Clock {
    start: Instant,
    ahead: u64,
}

impl Clock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            ahead: 0,
        }
    }

    fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .saturating_add(self.ahead)
    }
}

/// One room's machine and socket, on the room's own thread.
struct Driver {
    handle: String,
    peer: Peer,
    clock: Clock,
    net: Arc<dyn Net>,
    inputs: Receiver<Input>,
    /// For the connecting and token threads to answer on.
    back: Sender<Input>,
    socket: Option<Box<dyn Socket>>,
    /// The latest socket asked for; an older one's answer is dropped.
    attempt: u64,
    connecting: bool,
    open: Arc<Open>,
    watch: Arc<Watch>,
    tell: Tell,
    /// The relay asks for a token.
    token: bool,
    /// The connection's state, as the JS SDK names it.
    state: &'static str,
    /// Offline with a token this long: said as the relay not answering.
    unreachable_at: Option<Instant>,
}

impl Driver {
    fn run(mut self) {
        self.check_state();
        if self.token {
            self.fetch_token();
        }
        loop {
            self.pump();
            if matches!(self.peer.state(), State::Closed(_)) {
                break;
            }
            let wait = self.wait();
            let handled = self.socket.is_some()
                && match self.inputs.try_recv() {
                    Ok(input) => {
                        self.input(input);
                        true
                    }
                    Err(TryRecvError::Empty) => false,
                    Err(TryRecvError::Disconnected) => {
                        self.peer.leave();
                        true
                    }
                };
            if !handled {
                if let Some(socket) = self.socket.as_mut() {
                    let read = socket.read(wait.map_or(TICK, |wait| wait.min(TICK)));
                    self.read(read);
                } else {
                    let input = match wait {
                        Some(wait) => self.inputs.recv_timeout(wait),
                        None => self
                            .inputs
                            .recv()
                            .map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    match input {
                        Ok(input) => self.input(input),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => self.peer.leave(),
                    }
                }
            }
            self.timers();
        }
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
        // A token wait or stop of this room's ends with it.
        self.watch.end();
    }

    /// Until the next timer, if there is one.
    fn wait(&self) -> Option<Duration> {
        let now = self.clock.now();
        let machine = self
            .peer
            .poll_timeout()
            .map(|at| Duration::from_millis(at.saturating_sub(now)));
        let unreachable = self
            .unreachable_at
            .map(|at| at.saturating_duration_since(Instant::now()));
        [machine, unreachable].into_iter().flatten().min()
    }

    fn timers(&mut self) {
        let now = self.clock.now();
        if self.peer.poll_timeout().is_some_and(|at| now >= at) {
            self.peer.handle_timeout(now);
        }
        if self.unreachable_at.is_some_and(|at| Instant::now() >= at) {
            self.unreachable_at = None;
            self.watch
                .on_state(self.state, rules::UNREACHABLE_AFTER_MS, self.token);
        }
    }

    /// Everything the machine asks, done.
    fn pump(&mut self) {
        while let Some(output) = self.peer.poll_output() {
            match output {
                Output::Connect(url) => self.connect(url),
                Output::Send(frame) => {
                    if let Some(socket) = self.socket.as_mut() {
                        socket.send(Message::Binary(frame));
                    }
                }
                Output::Ping => {
                    if let Some(socket) = self.socket.as_mut() {
                        socket.send(Message::Text(wire::PING.to_owned()));
                    }
                }
                Output::Disconnect => {
                    self.connecting = false;
                    if let Some(mut socket) = self.socket.take() {
                        socket.close();
                    }
                }
                Output::Event(event) => self.event(event),
            }
        }
    }

    /// Open a socket to `url` on a thread of its own, so that a leave is
    /// not held up by a relay that does not answer.
    fn connect(&mut self, url: String) {
        self.attempt += 1;
        self.connecting = true;
        let (net, back, attempt) = (self.net.clone(), self.back.clone(), self.attempt);
        let spawned = thread::Builder::new()
            .name(format!("peerconnect {} connect", self.handle))
            .spawn(move || {
                let socket = net.connect(&url);
                let _ = back.send(Input::Opened { attempt, socket });
            });
        if spawned.is_err() {
            self.connecting = false;
            self.peer.handle_close(self.clock.now(), None);
        }
    }

    fn input(&mut self, input: Input) {
        let now = self.clock.now();
        match input {
            Input::Send(data) => {
                // Too large, too much unacknowledged, or closing: dropped,
                // as the web's transport drops a send that throws.
                let _ = self.peer.send(&data);
            }
            Input::Leave => self.peer.leave(),
            Input::End => self.peer.close(),
            Input::Token(token) => self.peer.set_token(now, Some(token)),
            Input::Now => self.reconnect_now(),
            Input::Opened { attempt, socket } => {
                if attempt != self.attempt || !self.connecting {
                    if let Ok(mut stale) = socket {
                        stale.close();
                    }
                    return;
                }
                self.connecting = false;
                match socket {
                    Ok(socket) => {
                        self.socket = Some(socket);
                        self.peer.handle_open(now);
                    }
                    Err(_) => self.peer.handle_close(now, None),
                }
            }
        }
    }

    /// The JS SDK's `reconnectNow` (r4C-15): offline and waiting to try the
    /// relay again — tries now, the clock moved on to that moment.
    fn reconnect_now(&mut self) {
        if self.state != "offline" || self.socket.is_some() || self.connecting {
            return;
        }
        let Some(at) = self.peer.poll_timeout() else {
            return;
        };
        let now = self.clock.now();
        if at > now {
            self.clock.ahead += at - now;
        }
        self.peer.handle_timeout(self.clock.now());
    }

    fn read(&mut self, read: Read) {
        let now = self.clock.now();
        match read {
            Read::Message(Message::Binary(frame)) => self.peer.handle_binary(now, &frame),
            Read::Message(Message::Text(text)) => self.peer.handle_text(now, &text),
            Read::Nothing => {}
            Read::Closed(code) => {
                self.socket = None;
                self.peer.handle_close(now, code);
            }
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::State(state) => {
                let reason = match state {
                    State::Offline => {
                        self.state = "offline";
                        None
                    }
                    State::Waiting => {
                        self.state = "waiting";
                        None
                    }
                    State::Connected => {
                        self.state = "connected";
                        None
                    }
                    State::Closed(reason) => {
                        self.state = "closed";
                        Some(match reason {
                            CloseReason::Left => "left",
                            CloseReason::Closed => "closed",
                            CloseReason::ClosedByPeer => "closed-by-peer",
                        })
                    }
                };
                self.check_state();
                (self.tell)(PeerEvent::State {
                    room: self.handle.clone(),
                    state: self.state.to_owned(),
                });
                if let Some(reason) = reason {
                    self.open.close_with(reason);
                }
            }
            Event::Message(bytes) => self.open.deliver(read_message(&bytes)),
            Event::Paired => (self.tell)(PeerEvent::Paired {
                room: self.handle.clone(),
                pairing: self.peer.room().to_string(),
            }),
            Event::Unauthorized => {
                if self.token {
                    // Before the next token is asked for: that one must not
                    // be the one just refused.
                    let refused = self.watch.unauthorized();
                    forget_relay_token(&*self.net, refused.as_deref());
                    self.fetch_token();
                }
            }
            Event::Delivered(_) | Event::Replaced => {}
        }
    }

    /// Where the way onto the relay stands, as the core reads the state;
    /// offline for the core's while, said as the relay not answering.
    fn check_state(&mut self) {
        self.unreachable_at = None;
        let accepted = self.watch.on_state(self.state, 0.0, self.token);
        if accepted || self.state == "closed" {
            return;
        }
        self.unreachable_at =
            Some(Instant::now() + Duration::from_secs_f64(rules::UNREACHABLE_AFTER_MS / 1000.0));
    }

    /// A token for the machine, on a thread of its own (one at a time).
    fn fetch_token(&self) {
        if !self.watch.begin_fetch() {
            return;
        }
        let (watch, net, back) = (self.watch.clone(), self.net.clone(), self.back.clone());
        let spawned = thread::Builder::new()
            .name(format!("peerconnect {} token", self.handle))
            .spawn(move || {
                let token = token_for(&watch, &*net);
                watch.end_fetch();
                if let Some(token) = token {
                    let _ = back.send(Input::Token(token));
                }
            });
        if spawned.is_err() {
            self.watch.end_fetch();
        }
    }
}

// ---------------------------------------------------------------------------
// The way onto the relay, and the token
// ---------------------------------------------------------------------------

/// One connection's way onto the relay, as the screen says it, with the
/// refusal count the core keeps for it. A wait it counts down is cut short,
/// and a stop undone, by [`Watch::retry`].
struct Watch {
    room: String,
    relay: String,
    tell: Tell,
    state: Mutex<WatchState>,
    woken: Condvar,
    /// Held while a status is stored and told, so they are told in order.
    telling: Mutex<()>,
}

#[derive(Default)]
struct WatchState {
    status: RelayStatus,
    /// Retry taps so far.
    wakes: u64,
    /// The room is gone: no more waiting.
    over: bool,
    refusals: Refusals,
    /// The token last handed to the machine.
    last: Option<String>,
    fetching: bool,
}

impl Watch {
    fn new(room: String, relay: String, tell: Tell) -> Self {
        Self {
            room,
            relay,
            tell,
            state: Mutex::new(WatchState::default()),
            woken: Condvar::new(),
            telling: Mutex::new(()),
        }
    }

    /// Change the state; a status it hands back is stored and told.
    fn update<T>(&self, change: impl FnOnce(&mut WatchState) -> (Option<RelayStatus>, T)) -> T {
        let _order = lock(&self.telling);
        let (status, answer) = {
            let mut state = lock(&self.state);
            let (status, answer) = change(&mut state);
            if let Some(status) = &status {
                state.status = status.clone();
            }
            (status, answer)
        };
        if let Some(status) = status {
            (self.tell)(PeerEvent::Relay {
                room: self.room.clone(),
                status,
            });
        }
        answer
    }

    fn set(&self, status: RelayStatus) {
        self.update(|_| (Some(status), ()));
    }

    fn refusals<T>(&self, ask: impl FnOnce(&mut Refusals) -> T) -> T {
        ask(&mut lock(&self.state).refusals)
    }

    /// The relay refused the token in use: counted; the token is handed back.
    fn unauthorized(&self) -> Option<String> {
        let mut state = lock(&self.state);
        state.refusals.unauthorized();
        state.last.clone()
    }

    fn issued(&self, token: &str, source: TokenSource) {
        let mut state = lock(&self.state);
        state.last = Some(token.to_owned());
        state.refusals.issued(source);
    }

    fn begin_fetch(&self) -> bool {
        let mut state = lock(&self.state);
        !std::mem::replace(&mut state.fetching, true) && !state.over
    }

    fn end_fetch(&self) {
        lock(&self.state).fetching = false;
    }

    /// The core's reading of the connection's `state` (offline `offline_ms`):
    /// a new status is told; on the relay, the refusals are forgotten.
    /// Whether it is on the relay.
    fn on_state(&self, state: &str, offline_ms: f64, token: bool) -> bool {
        self.update(|watch| {
            let (status, accepted) = on_state(&watch.status, state, offline_ms, &self.relay);
            if accepted && token {
                watch.refusals.accepted();
            }
            ((status != watch.status).then_some(status), accepted)
        })
    }

    /// Wait `ms`, said as retrying for `why` — less, if Retry is tapped.
    /// `false`: the room is gone.
    fn wait(&self, why: RelayTrouble, detail: &str, ms: u64) -> bool {
        let deadline = Instant::now() + Duration::from_millis(ms);
        let at = epoch_ms() + ms as f64;
        let wakes = self.update(|watch| {
            let status = RelayStatus::Retrying {
                why,
                detail: detail.to_owned(),
                at: Some(at),
            };
            (Some(status), watch.wakes)
        });
        let mut state = lock(&self.state);
        while !state.over && state.wakes == wakes {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            state = self
                .woken
                .wait_timeout(state, left)
                .map_or_else(|e| e.into_inner().0, |(state, _)| state);
        }
        !state.over
    }

    /// Stop: said as failed for `why`, until Retry is tapped. `false`: the
    /// room is gone.
    fn stop(&self, why: RelayTrouble, detail: &str) -> bool {
        let wakes = self.update(|watch| {
            let status = RelayStatus::Failed {
                why,
                detail: detail.to_owned(),
            };
            (Some(status), watch.wakes)
        });
        {
            let mut state = lock(&self.state);
            while !state.over && state.wakes == wakes {
                state = self
                    .woken
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if state.over {
                return false;
            }
        }
        self.set(RelayStatus::Connecting);
        true
    }

    /// Try again now: a wait cut short, a stop undone.
    fn retry(&self) {
        lock(&self.state).wakes += 1;
        self.woken.notify_all();
    }

    /// The room is gone.
    fn end(&self) {
        lock(&self.state).over = true;
        self.woken.notify_all();
    }

    fn is_over(&self) -> bool {
        lock(&self.state).over
    }
}

/// The core's `relay.onState`: the status, and whether the connection is
/// on the relay (its refusals then forgotten). Asked through the rules
/// door, which answers both; an answer that does not read keeps the status.
fn on_state(
    status: &RelayStatus,
    state: &str,
    offline_ms: f64,
    relay: &str,
) -> (RelayStatus, bool) {
    let args = json!({ "status": status, "state": state, "offlineMs": offline_ms, "relay": relay });
    vela_core::multisig_rules::call_json("relay.onState", &args)
        .and_then(Result::ok)
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|answer| {
            let status = serde_json::from_value(answer.get("status")?.clone()).ok()?;
            Some((status, answer.get("accepted")?.as_bool()?))
        })
        .unwrap_or_else(|| (status.clone(), false))
}

/// The token for the machine, as the core schedules it: a refusal waited
/// out (or stopped, until Retry), then the kept one or a new one, with the
/// waits and the stop when none comes. `None`: the room is gone.
fn token_for(watch: &Watch, net: &dyn Net) -> Option<String> {
    let mut before = watch.refusals(Refusals::before_token);
    loop {
        match before {
            Before::Go { .. } => break,
            Before::Wait { ms } => {
                if !watch.wait(RelayTrouble::Refused, "HTTP 4403", ms) {
                    return None;
                }
            }
            Before::Stop => {
                if !watch.stop(RelayTrouble::Refused, "HTTP 4403") {
                    return None;
                }
                watch.refusals(Refusals::retried);
            }
        }
        before = watch.refusals(Refusals::before_token);
    }
    // The last chance before stopping is the trial's, once.
    let trial = matches!(before, Before::Go { trial: true });
    let mut tries = 0;
    loop {
        if watch.is_over() {
            return None;
        }
        watch.set(RelayStatus::Connecting);
        match relay_token(net, trial) {
            Ok((token, source)) => {
                watch.issued(&token, source);
                return Some(token);
            }
            Err(detail) => match rules::no_token_step(tries) {
                Before::Wait { ms } => {
                    if !watch.wait(RelayTrouble::NoToken, &detail, ms) {
                        return None;
                    }
                    tries += 1;
                }
                _ => {
                    if !watch.stop(RelayTrouble::NoToken, &detail) {
                        return None;
                    }
                    tries = 0;
                }
            },
        }
    }
}

/// Two rooms opening at once share one request.
static ASKING: Mutex<()> = Mutex::new(());

/// A token for the public relay: the one kept, if the core finds it still
/// good, else a new one from Vela's site, else a trial one (`trial`: the
/// trial's, unless the one kept is the trial's already). `Err`: what each
/// place answered.
fn relay_token(net: &dyn Net, trial: bool) -> Result<(String, TokenSource), String> {
    if let Some(usable) = rules::usable(kept_token(net).as_ref(), now_s(), trial) {
        return Ok(usable);
    }
    let _one = lock(&ASKING);
    // Another room may have just asked.
    if let Some(usable) = rules::usable(kept_token(net).as_ref(), now_s(), trial) {
        return Ok(usable);
    }
    let endpoint = token_endpoint(net);
    let site = if trial {
        None
    } else {
        Some(ask(
            net,
            &endpoint,
            Some(&json!({ "device": device_id(net) })),
        ))
    };
    let fresh = match site {
        Some(Ok(kept)) => KeptToken {
            source: Some(TokenSource::Site),
            ..kept
        },
        site => match ask(net, rules::TRIAL_ENDPOINT, None) {
            Ok(kept) => KeptToken {
                source: Some(TokenSource::Trial),
                ..kept
            },
            Err(trial) => {
                let site = site.and_then(Result::err);
                return Err(rules::no_token_detail(&endpoint, site.as_deref(), &trial));
            }
        },
    };
    net.store(TOKEN_KEY, serde_json::to_value(&fresh).ok());
    Ok((fresh.token, fresh.source.unwrap_or(TokenSource::Site)))
}

/// One endpoint asked; what it answered is read by the core.
fn ask(net: &dyn Net, url: &str, body: Option<&Value>) -> Result<KeptToken, String> {
    let (status, text) = net.post(url, body);
    rules::token_answer(status, &text)
}

/// The token kept, as stored (an object, or its JSON text).
fn kept_token(net: &dyn Net) -> Option<KeptToken> {
    match net.load(TOKEN_KEY)? {
        Value::String(text) => serde_json::from_str(&text).ok(),
        value => serde_json::from_value(value).ok(),
    }
}

/// The relay refused `refused` (any, when unknown): the core says whether
/// the kept token goes, so the next ask gets a new one.
fn forget_relay_token(net: &dyn Net, refused: Option<&str>) {
    if rules::forgets(kept_token(net).as_ref(), refused) {
        net.store(TOKEN_KEY, None);
    }
}

/// This device's ID at Vela's site: 32 random hex characters, made once.
fn device_id(net: &dyn Net) -> String {
    let kept = stored_text(net, DEVICE_KEY).filter(|id| {
        (26..=64).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    if let Some(id) = kept {
        return id;
    }
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // No system generator: a randomly keyed hash, still unguessable
        // enough for an ID that only names this device to the site.
        for half in bytes.chunks_mut(8) {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u128(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            half.copy_from_slice(&hasher.finish().to_le_bytes());
        }
    }
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    net.store(DEVICE_KEY, Some(Value::String(id.clone())));
    id
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashSet;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::AtomicBool;

    use peerconnect::relay::{Action, Conn, Incoming, room_from_path};
    use peerconnect::{Role, RoomId};

    use super::*;

    /// Longest a test waits for one thing to happen.
    const WAIT: Duration = Duration::from_secs(30);
    const PLAIN: &str = "ws://mem.test";

    // -- An in-memory relay, deciding as PeerConnect's own does ------------

    /// The world a [`MemNet`] is: a relay (`peerconnect::relay`'s decisions,
    /// seated as `peerconnect-relay` seats), token endpoints, a store.
    #[derive(Default)]
    struct World {
        seats: Mutex<Seats>,
        /// When some: the relay requires one of these tokens.
        admits: Option<Mutex<HashSet<String>>>,
        /// Nothing answers at the relay.
        down: AtomicBool,
        /// What each token endpoint answers (none: nothing answers).
        answers: Mutex<HashMap<String, (Option<u16>, String)>>,
        posts: Mutex<Vec<(String, Option<Value>)>>,
        urls: Mutex<Vec<String>>,
        store: Mutex<HashMap<String, Value>>,
    }

    type Seat = Option<(u64, Sender<Read>)>;

    #[derive(Default)]
    struct Seats {
        next: u64,
        rooms: HashMap<RoomId, [Seat; 2]>,
    }

    fn binary(frame: &[u8]) -> Read {
        Read::Message(Message::Binary(frame.to_vec()))
    }

    impl World {
        /// Seat `id` in `role`, replacing whoever held it; both sides told.
        fn seat(&self, room: RoomId, role: Role, id: u64, tx: Sender<Read>) {
            let mut seats = lock(&self.seats);
            let room = seats.rooms.entry(room).or_default();
            let partner = room[role.other().index()].clone();
            let _ = tx.send(binary(&wire::presence(partner.is_some())));
            if let Some((_, old)) = room[role.index()].replace((id, tx)) {
                let _ = old.send(Read::Closed(Some(wire::close::REPLACED)));
            }
            if let Some((_, partner)) = partner {
                let _ = partner.send(binary(&wire::presence(true)));
            }
        }

        fn forward(&self, room: RoomId, role: Role, id: u64, frame: Vec<u8>) {
            let seats = lock(&self.seats);
            let Some(room) = seats.rooms.get(&room) else {
                return;
            };
            if room[role.index()]
                .as_ref()
                .is_some_and(|(held, _)| *held == id)
                && let Some((_, partner)) = &room[role.other().index()]
            {
                let _ = partner.send(Read::Message(Message::Binary(frame)));
            }
        }

        fn leave(&self, room: RoomId, id: u64) {
            let mut seats = lock(&self.seats);
            let Some(seated) = seats.rooms.get_mut(&room) else {
                return;
            };
            for role in [Role::Inviter, Role::Joiner] {
                if seated[role.index()]
                    .as_ref()
                    .is_some_and(|(held, _)| *held == id)
                {
                    seated[role.index()] = None;
                    if let Some((_, partner)) = &seated[role.other().index()] {
                        let _ = partner.send(binary(&wire::presence(false)));
                    }
                }
            }
        }
    }

    #[derive(Clone, Default)]
    struct MemNet(Arc<World>);

    impl MemNet {
        fn plain() -> Self {
            Self::default()
        }

        /// A relay that admits only `tokens`.
        fn admitting(tokens: &[&str]) -> Self {
            Self(Arc::new(World {
                admits: Some(Mutex::new(tokens.iter().map(|t| (*t).to_owned()).collect())),
                ..World::default()
            }))
        }

        fn answer(&self, url: &str, status: u16, body: &Value) {
            lock(&self.0.answers).insert(url.to_owned(), (Some(status), body.to_string()));
        }

        fn stored(&self, key: &str) -> Option<Value> {
            lock(&self.0.store).get(key).cloned()
        }

        fn urls(&self) -> Vec<String> {
            lock(&self.0.urls).clone()
        }
    }

    struct MemSocket {
        world: Arc<World>,
        id: u64,
        room: RoomId,
        conn: Conn,
        tx: Sender<Read>,
        rx: Receiver<Read>,
    }

    impl Socket for MemSocket {
        fn send(&mut self, message: Message) {
            let action = match &message {
                Message::Binary(frame) => self.conn.receive(&self.room, Incoming::Binary(frame)),
                Message::Text(text) => self.conn.receive(&self.room, Incoming::Text(text)),
            };
            match action {
                Action::Pong => {
                    let _ = self
                        .tx
                        .send(Read::Message(Message::Text(wire::PONG.to_owned())));
                }
                Action::Join(role) => {
                    self.conn = Conn::Seated(role);
                    self.world.seat(self.room, role, self.id, self.tx.clone());
                }
                Action::Forward => {
                    if let (Conn::Seated(role), Message::Binary(frame)) = (self.conn, message) {
                        self.world.forward(self.room, role, self.id, frame);
                    }
                }
                Action::Close(code) => {
                    self.world.leave(self.room, self.id);
                    let _ = self.tx.send(Read::Closed(Some(code)));
                }
            }
        }

        fn read(&mut self, wait: Duration) -> Read {
            match self.rx.recv_timeout(wait) {
                Ok(read) => read,
                Err(RecvTimeoutError::Timeout) => Read::Nothing,
                Err(RecvTimeoutError::Disconnected) => Read::Closed(None),
            }
        }

        fn close(&mut self) {
            self.world.leave(self.room, self.id);
        }
    }

    impl Drop for MemSocket {
        fn drop(&mut self) {
            self.world.leave(self.room, self.id);
        }
    }

    impl Net for MemNet {
        fn connect(&self, url: &str) -> Result<Box<dyn Socket>, String> {
            lock(&self.0.urls).push(url.to_owned());
            if self.0.down.load(Ordering::SeqCst) {
                return Err("unreachable".to_owned());
            }
            let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
            let room = room_from_path(parsed.path()).ok_or("not found")?;
            let id = {
                let mut seats = lock(&self.0.seats);
                seats.next += 1;
                seats.next
            };
            let (tx, rx) = mpsc::channel();
            let (conn, hello) = Conn::new();
            let admitted = self.0.admits.as_ref().is_none_or(|admits| {
                parsed
                    .query_pairs()
                    .find(|(key, _)| key == "token")
                    .is_some_and(|(_, token)| lock(admits).contains(token.as_ref()))
            });
            // Accepted, then closed: a refused token is a close code.
            let _ = tx.send(if admitted {
                binary(&hello)
            } else {
                Read::Closed(Some(wire::close::TOKEN))
            });
            Ok(Box::new(MemSocket {
                world: self.0.clone(),
                id,
                room,
                conn,
                tx,
                rx,
            }))
        }

        fn post(&self, url: &str, body: Option<&Value>) -> (Option<u16>, String) {
            lock(&self.0.posts).push((url.to_owned(), body.cloned()));
            lock(&self.0.answers)
                .get(url)
                .cloned()
                .unwrap_or((None, String::new()))
        }

        fn load(&self, key: &str) -> Option<Value> {
            self.stored(key)
        }

        fn store(&self, key: &str, value: Option<Value>) {
            let mut store = lock(&self.0.store);
            match value {
                Some(value) => store.insert(key.to_owned(), value),
                None => store.remove(key),
            };
        }
    }

    // -- What a machine hears ---------------------------------------------

    struct Heard {
        events: Mutex<Receiver<PeerEvent>>,
        /// Heard, and not yet found by an [`Heard::until`] (rooms' threads
        /// tell in any order between them).
        pending: Mutex<VecDeque<PeerEvent>>,
        seen: Mutex<Vec<PeerEvent>>,
    }

    fn heard() -> (Tell, Arc<Heard>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let tell: Tell = Arc::new(move |event| {
            let _ = lock(&tx).send(event);
        });
        (
            tell,
            Arc::new(Heard {
                events: Mutex::new(rx),
                pending: Mutex::new(VecDeque::new()),
                seen: Mutex::new(Vec::new()),
            }),
        )
    }

    impl Heard {
        /// The first event `wanted` finds — heard already, or within [`WAIT`].
        fn until(&self, what: &str, wanted: impl Fn(&PeerEvent) -> bool) -> PeerEvent {
            {
                let mut pending = lock(&self.pending);
                if let Some(at) = pending.iter().position(&wanted) {
                    return pending.remove(at).unwrap();
                }
            }
            let deadline = Instant::now() + WAIT;
            let events = lock(&self.events);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match events.recv_timeout(left) {
                    Ok(event) => {
                        lock(&self.seen).push(event.clone());
                        if wanted(&event) {
                            return event;
                        }
                        lock(&self.pending).push_back(event);
                    }
                    Err(_) => panic!("never heard {what}; heard {:#?}", lock(&self.seen)),
                }
            }
        }

        fn state(&self, room: &str, state: &str) {
            self.until(&format!("{room} {state}"), |event| {
                matches!(event, PeerEvent::State { room: r, state: s } if r == room && s == state)
            });
        }

        fn pairing(&self, room: &str) -> String {
            match self.until(
                &format!("{room} paired"),
                |event| matches!(event, PeerEvent::Paired { room: r, .. } if r == room),
            ) {
                PeerEvent::Paired { pairing, .. } => pairing,
                other => panic!("not a pairing: {other:?}"),
            }
        }

        fn relay(
            &self,
            room: &str,
            what: &str,
            wanted: impl Fn(&RelayStatus) -> bool,
        ) -> RelayStatus {
            match self.until(what, |event| {
                matches!(event, PeerEvent::Relay { room: r, status } if r == room && wanted(status))
            }) {
                PeerEvent::Relay { status, .. } => status,
                other => panic!("not a relay status: {other:?}"),
            }
        }
    }

    /// `receive`, failing the test rather than blocking it forever.
    fn receive(rooms: &Arc<PeerRooms>, room: &str) -> Arrival {
        let (tx, rx) = mpsc::channel();
        let (rooms, room) = (rooms.clone(), room.to_owned());
        thread::spawn(move || {
            let _ = tx.send(rooms.receive(&room));
        });
        rx.recv_timeout(WAIT)
            .unwrap_or_else(|_| panic!("nothing arrived within {WAIT:?}"))
    }

    fn open_box(room: &str) -> Arc<Open> {
        let (inputs, _) = mpsc::channel();
        Arc::new(Open {
            inbox: Mutex::new(Inbox::default()),
            arrived: Condvar::new(),
            inputs,
            watch: Arc::new(Watch::new(
                room.to_owned(),
                PLAIN.to_owned(),
                Arc::new(|_| {}),
            )),
        })
    }

    // -- The inbox, the waiter, the close ---------------------------------

    #[test]
    fn a_message_reads_as_json_else_as_text_and_a_string_goes_as_its_text() {
        assert_eq!(read_message(br#"{"a":[1,"b"]}"#), json!({ "a": [1, "b"] }));
        assert_eq!(read_message(b"\"quoted\""), json!("quoted"));
        assert_eq!(read_message(b"12"), json!(12));
        assert_eq!(read_message(b"plain words"), json!("plain words"));
        assert_eq!(read_message(&[0x68, 0xff, 0x69]), json!("h\u{fffd}i"));
        assert_eq!(write_message(&json!("plain words")), b"plain words");
        assert_eq!(write_message(&json!({ "a": 1 })), br#"{"a":1}"#);
    }

    #[test]
    fn a_message_with_nobody_waiting_is_queued_and_a_waiter_gets_the_next() {
        let open = open_box("room-1");
        open.deliver(json!(1));
        open.deliver(json!(2));
        assert_eq!(open.receive(), Arrival::Message(json!(1)));
        assert_eq!(open.receive(), Arrival::Message(json!(2)));

        let waiter = {
            let open = open.clone();
            thread::spawn(move || open.receive())
        };
        thread::sleep(Duration::from_millis(50));
        open.deliver(json!(3));
        assert_eq!(waiter.join().unwrap(), Arrival::Message(json!(3)));
    }

    #[test]
    fn a_close_comes_after_the_queue_to_the_waiter_and_to_every_receive_after() {
        let open = open_box("room-1");
        open.deliver(json!("before"));
        open.close_with("closed-by-peer");
        assert_eq!(open.receive(), Arrival::Message(json!("before")));
        assert_eq!(open.receive(), Arrival::Closed("closed-by-peer".to_owned()));
        assert_eq!(open.receive(), Arrival::Closed("closed-by-peer".to_owned()));

        let open = open_box("room-2");
        let waiter = {
            let open = open.clone();
            thread::spawn(move || open.receive())
        };
        thread::sleep(Duration::from_millis(50));
        open.close_with("left");
        assert_eq!(waiter.join().unwrap(), Arrival::Closed("left".to_owned()));
    }

    #[test]
    fn closing_wakes_the_waiter_and_forgets_the_room() {
        let (tell, heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(MemNet::plain()));
        let opened = rooms.open(Some(PLAIN), None, None).unwrap();
        assert_eq!(opened.room, "room-1");
        heard.state("room-1", "waiting");

        let waiter = {
            let rooms = rooms.clone();
            thread::spawn(move || rooms.receive("room-1"))
        };
        thread::sleep(Duration::from_millis(50));
        rooms.close("room-1", false);
        assert_eq!(waiter.join().unwrap(), Arrival::Closed("left".to_owned()));
        // Gone: a receive is closed, a send goes nowhere.
        assert_eq!(
            rooms.receive("room-1"),
            Arrival::Closed("closed".to_owned())
        );
        rooms.send("room-1", &json!("nobody"));
        heard.state("room-1", "closed");
    }

    #[test]
    fn leave_all_wakes_every_waiter() {
        let (tell, heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(MemNet::plain()));
        let first = rooms.open(Some(PLAIN), None, None).unwrap().room;
        let second = rooms.open(Some(PLAIN), None, None).unwrap().room;
        assert_eq!((first.as_str(), second.as_str()), ("room-1", "room-2"));
        let waiters: Vec<_> = [first.clone(), second.clone()]
            .into_iter()
            .map(|room| {
                let rooms = rooms.clone();
                thread::spawn(move || rooms.receive(&room))
            })
            .collect();
        thread::sleep(Duration::from_millis(50));
        rooms.leave_all();
        for waiter in waiters {
            assert_eq!(waiter.join().unwrap(), Arrival::Closed("left".to_owned()));
        }
        heard.state(&first, "closed");
        heard.state(&second, "closed");
    }

    #[test]
    fn an_unknown_room_is_closed_and_a_room_that_cannot_open_says_why() {
        let (tell, _heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(MemNet::plain()));
        assert_eq!(
            rooms.receive("room-9"),
            Arrival::Closed("closed".to_owned())
        );
        assert!(rooms.open(None, Some("not an invite"), None).is_err());
        assert!(rooms.open(None, None, Some("not a record")).is_err());
        assert!(rooms.open(Some("https://not.a.relay"), None, None).is_err());
        // The relay the device names, else the public one.
        let net = MemNet::plain();
        net.store(RELAY_KEY, Some(json!("ws://mine.test")));
        let rooms = PeerRooms::with_net(heard().0, Arc::new(net));
        let opened = rooms.open(None, None, None).unwrap();
        assert!(opened.invite.unwrap().starts_with("ws://mine.test#"));
    }

    // -- Two sides of a room ----------------------------------------------

    /// One creates, the other joins with the invite; they talk both ways,
    /// pair, reopen from their pairings, and one ends the room.
    fn talk_pair_reopen_and_end(net_a: Arc<dyn Net>, net_b: Arc<dyn Net>, relay: &str) {
        let (tell_a, heard_a) = heard();
        let (tell_b, heard_b) = heard();
        let a = PeerRooms::with_net(tell_a, net_a);
        let b = PeerRooms::with_net(tell_b, net_b);

        let created = a.open(Some(relay), None, None).unwrap();
        let invite = created.invite.clone().expect("a fresh room has an invite");
        assert!(invite.starts_with(&format!("{relay}#")));
        let joined = b.open(None, Some(&invite), None).unwrap();
        assert_eq!(joined.invite, None);
        let (ra, rb) = (created.room.clone(), joined.room.clone());
        heard_a.state(&ra, "connected");
        heard_b.state(&rb, "connected");

        a.send(&ra, &json!({ "hello": "from a", "n": [1, 2] }));
        assert_eq!(
            receive(&b, &rb),
            Arrival::Message(json!({ "hello": "from a", "n": [1, 2] }))
        );
        b.send(&rb, &json!("plain words"));
        b.send(&rb, &json!({ "second": true }));
        assert_eq!(receive(&a, &ra), Arrival::Message(json!("plain words")));
        assert_eq!(
            receive(&a, &ra),
            Arrival::Message(json!({ "second": true }))
        );

        let pairing_a = heard_a.pairing(&ra);
        let pairing_b = heard_b.pairing(&rb);
        a.close(&ra, false);
        b.close(&rb, false);
        assert_eq!(receive(&a, &ra), Arrival::Closed("closed".to_owned()));

        // The pairings reopen the room: no invite, and the two meet again.
        let again_a = a.open(None, None, Some(&pairing_a)).unwrap();
        let again_b = b.open(None, None, Some(&pairing_b)).unwrap();
        assert_eq!(
            (again_a.invite.as_ref(), again_b.invite.as_ref()),
            (None, None)
        );
        assert_eq!(again_a.pairing.as_deref(), Some(pairing_a.as_str()));
        heard_a.state(&again_a.room, "connected");
        heard_b.state(&again_b.room, "connected");
        b.send(&again_b.room, &json!({ "back": "again" }));
        assert_eq!(
            receive(&a, &again_a.room),
            Arrival::Message(json!({ "back": "again" }))
        );

        // One side ends it: the other hears the room is over.
        a.close(&again_a.room, true);
        assert_eq!(
            receive(&b, &again_b.room),
            Arrival::Closed("closed-by-peer".to_owned())
        );
        heard_a.state(&again_a.room, "closed");
        heard_b.state(&again_b.room, "closed");
    }

    #[test]
    fn two_sides_talk_pair_reopen_and_end() {
        let net = MemNet::plain();
        talk_pair_reopen_and_end(Arc::new(net.clone()), Arc::new(net.clone()), PLAIN);
        assert!(net.urls().iter().all(|url| !url.contains("token=")));
    }

    // -- The relay's token ------------------------------------------------

    fn a_day_from_now() -> f64 {
        now_s() + 86_400.0
    }

    #[test]
    fn a_refused_token_is_forgotten_waited_out_and_a_new_one_used() {
        let net = MemNet::admitting(&["good"]);
        net.store(
            TOKEN_KEY,
            Some(json!({ "token": "old", "expiresAt": a_day_from_now(), "source": "site" })),
        );
        net.answer(
            rules::TOKEN_ENDPOINT,
            200,
            &json!({ "token": "good", "expires_at": a_day_from_now() }),
        );
        let (tell, heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(net.clone()));
        // No relay named: the public one, which asks for a token.
        let opened = rooms.open(None, None, None).unwrap();
        assert!(
            opened
                .invite
                .unwrap()
                .starts_with(&format!("{}#", rules::DEFAULT_RELAY))
        );

        let waiting = heard.relay(&opened.room, "refused, waiting", |status| {
            matches!(
                status,
                RelayStatus::Retrying {
                    why: RelayTrouble::Refused,
                    ..
                }
            )
        });
        assert!(matches!(
            waiting,
            RelayStatus::Retrying { ref detail, at: Some(_), .. } if detail == "HTTP 4403"
        ));
        // The refused token is gone before the next is asked for.
        assert_eq!(net.stored(TOKEN_KEY), None);
        rooms.retry(&opened.room);
        heard.relay(&opened.room, "up", |status| *status == RelayStatus::Up);
        heard.state(&opened.room, "waiting");

        let posts = lock(&net.0.posts).clone();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, rules::TOKEN_ENDPOINT);
        let device = net.stored(DEVICE_KEY).unwrap();
        assert_eq!(device.as_str().unwrap().len(), 32);
        assert_eq!(posts[0].1, Some(json!({ "device": device })));
        let kept = net.stored(TOKEN_KEY).unwrap();
        assert_eq!(
            (kept["token"].clone(), kept["source"].clone()),
            (json!("good"), json!("site"))
        );
        let urls = net.urls();
        assert!(urls.first().unwrap().ends_with("?token=old"));
        assert!(urls.last().unwrap().ends_with("?token=good"));
        rooms.close(&opened.room, false);
    }

    #[test]
    fn no_token_from_anywhere_is_said_and_retry_asks_again() {
        let net = MemNet::admitting(&["trial-token"]);
        let (tell, heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(net.clone()));
        let opened = rooms.open(None, None, None).unwrap();
        let said = heard.relay(&opened.room, "no token", |status| {
            matches!(
                status,
                RelayStatus::Retrying {
                    why: RelayTrouble::NoToken,
                    ..
                }
            )
        });
        let detail = rules::no_token_detail(rules::TOKEN_ENDPOINT, Some("no answer"), "no answer");
        assert!(matches!(said, RelayStatus::Retrying { detail: ref d, .. } if *d == detail));

        // The trial answers now (Vela's site still does not).
        net.answer(
            rules::TRIAL_ENDPOINT,
            200,
            &json!({ "token": "trial-token", "expires_at": a_day_from_now() }),
        );
        rooms.retry(&opened.room);
        heard.relay(&opened.room, "up", |status| *status == RelayStatus::Up);
        assert_eq!(net.stored(TOKEN_KEY).unwrap()["source"], json!("trial"));
        rooms.close(&opened.room, false);
    }

    #[test]
    fn a_relay_that_does_not_answer_is_said_unreachable_and_retry_tries_at_once() {
        let net = MemNet::plain();
        net.0.down.store(true, Ordering::SeqCst);
        let (tell, heard) = heard();
        let rooms = PeerRooms::with_net(tell, Arc::new(net.clone()));
        let opened = rooms.open(Some(PLAIN), None, None).unwrap();
        let said = heard.relay(&opened.room, "unreachable", |status| {
            matches!(
                status,
                RelayStatus::Retrying {
                    why: RelayTrouble::Unreachable,
                    ..
                }
            )
        });
        assert!(
            matches!(said, RelayStatus::Retrying { ref detail, at: None, .. } if detail == "mem.test")
        );

        net.0.down.store(false, Ordering::SeqCst);
        let tried = net.urls().len();
        rooms.retry(&opened.room);
        heard.relay(&opened.room, "up", |status| *status == RelayStatus::Up);
        assert!(net.urls().len() > tried);
        rooms.close(&opened.room, false);
    }

    // -- Against the real relay -------------------------------------------

    /// A `peerconnect-relay` on a free local port, stopped when dropped.
    struct RealRelay {
        child: Child,
        url: String,
    }

    impl Drop for RealRelay {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// The binary `PEERCONNECT_RELAY_BIN` names, running; `None` (said) when
    /// there is none.
    fn real_relay() -> Option<RealRelay> {
        let Some(bin) = std::env::var_os("PEERCONNECT_RELAY_BIN").map(PathBuf::from) else {
            eprintln!("skipped: PEERCONNECT_RELAY_BIN does not name a peerconnect-relay binary");
            return None;
        };
        if !bin.is_file() {
            eprintln!("skipped: no peerconnect-relay at {}", bin.display());
            return None;
        }
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = Command::new(bin)
            .env("PEERCONNECT_LISTEN", format!("127.0.0.1:{port}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let relay = RealRelay {
            child,
            url: format!("ws://127.0.0.1:{port}"),
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(Instant::now() < deadline, "the relay did not start");
            thread::sleep(Duration::from_millis(50));
        }
        Some(relay)
    }

    /// Plain TCP (no proxy) and a store in memory.
    #[derive(Default)]
    struct TcpNet {
        store: Mutex<HashMap<String, Value>>,
    }

    impl Net for TcpNet {
        fn connect(&self, url: &str) -> Result<Box<dyn Socket>, String> {
            let socket = WsSocket::open(url, |host, port, _| {
                TcpStream::connect((host, port)).map_err(|e| e.to_string())
            })?;
            Ok(Box::new(socket))
        }

        fn post(&self, _url: &str, _body: Option<&Value>) -> (Option<u16>, String) {
            (None, String::new())
        }

        fn load(&self, key: &str) -> Option<Value> {
            lock(&self.store).get(key).cloned()
        }

        fn store(&self, key: &str, value: Option<Value>) {
            let mut store = lock(&self.store);
            match value {
                Some(value) => store.insert(key.to_owned(), value),
                None => store.remove(key),
            };
        }
    }

    #[test]
    #[ignore = "needs peerconnect-relay: PEERCONNECT_RELAY_BIN=/path/to/peerconnect-relay cargo test multisig::peer -- --ignored"]
    fn real_relay_two_desktops_talk_pair_reopen_and_end() {
        let Some(relay) = real_relay() else {
            return;
        };
        talk_pair_reopen_and_end(
            Arc::new(TcpNet::default()),
            Arc::new(TcpNet::default()),
            &relay.url,
        );
    }

    /// The web's connect page is a browser running the vendored JS client
    /// (`app-web/…/vendor/peerconnect`); here it is that same client in
    /// Node, joining a room the desktop created.
    #[test]
    #[ignore = "needs peerconnect-relay and Node 22+: PEERCONNECT_RELAY_BIN=/path/to/peerconnect-relay cargo test multisig::peer -- --ignored"]
    fn real_relay_the_web_client_joins_a_room_the_desktop_created() {
        // Says hello once connected, echoes what it gets, ends the room when told to.
        const PAGE: &str = r#"
            const { pathToFileURL } = await import('node:url');
            const { connect, Room } = await import(pathToFileURL(process.env.PC_INDEX).href);
            const conn = connect(Room.join(process.env.PC_INVITE));
            const out = (o) => console.log(JSON.stringify(o));
            let said = false;
            conn.addEventListener('paired', () => out({ paired: true }));
            conn.addEventListener('state', () => {
                out({ state: conn.state, reason: conn.reason ?? null });
                if (conn.state === 'connected' && !said) {
                    said = true;
                    conn.send(JSON.stringify({ from: 'js' }));
                }
                if (conn.state === 'closed') setTimeout(() => process.exit(0), 50);
            });
            conn.addEventListener('message', (event) => {
                const text = new TextDecoder().decode(event.data);
                let value;
                try { value = JSON.parse(text); } catch { value = text; }
                if (value && value.end) { conn.close(); return; }
                conn.send(JSON.stringify({ echo: value }));
            });
            setTimeout(() => { out({ timedOut: true }); process.exit(2); }, 30000);
        "#;

        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("skipped: no node on PATH");
            return;
        }
        let Some(relay) = real_relay() else {
            return;
        };
        let index = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../app-web/vela-wallet/src/lib/vendor/peerconnect/index.js")
            .canonicalize()
            .unwrap();
        let (tell, heard) = heard();
        let desktop = PeerRooms::with_net(tell, Arc::new(TcpNet::default()));
        let created = desktop.open(Some(&relay.url), None, None).unwrap();
        let room = created.room.clone();

        let mut page = Command::new("node")
            .args(["--input-type=module", "-e", PAGE])
            .env("PC_INDEX", &index)
            .env("PC_INVITE", created.invite.as_deref().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        heard.state(&room, "connected");
        assert_eq!(
            receive(&desktop, &room),
            Arrival::Message(json!({ "from": "js" }))
        );
        desktop.send(&room, &json!({ "n": 1, "list": ["a"] }));
        assert_eq!(
            receive(&desktop, &room),
            Arrival::Message(json!({ "echo": { "n": 1, "list": ["a"] } }))
        );
        desktop.send(&room, &json!("words, not JSON"));
        assert_eq!(
            receive(&desktop, &room),
            Arrival::Message(json!({ "echo": "words, not JSON" }))
        );
        heard.pairing(&room);
        // The page ends the room.
        desktop.send(&room, &json!({ "end": true }));
        assert_eq!(
            receive(&desktop, &room),
            Arrival::Closed("closed-by-peer".to_owned())
        );
        heard.state(&room, "closed");

        let deadline = Instant::now() + WAIT;
        let status = loop {
            if let Some(status) = page.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "the page did not finish");
            thread::sleep(Duration::from_millis(50));
        };
        let output = page.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&output.stdout);
        assert!(
            status.success(),
            "the page: {said}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            said.contains(r#"{"paired":true}"#),
            "the page never paired: {said}"
        );
        assert!(
            said.contains(r#"{"state":"closed","reason":"closed"}"#),
            "{said}"
        );
    }
}
