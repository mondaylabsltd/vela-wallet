//! A Tokio driver for [`Peer`]: real WebSockets (with TLS for `wss://`),
//! real timers, and an async API.
//!
//! ```no_run
//! # async fn demo(invite: &str) -> Result<(), Box<dyn std::error::Error>> {
//! use peerconnect::{Event, Room, State, client};
//!
//! let room = Room::join(&invite.parse()?);
//! // Persist `room` now: it holds the identity the inviter will pin.
//! let mut conn = client::connect(room);
//! conn.send(b"hello").await?; // queued until the peer is reachable
//! while let Some(event) = conn.next().await {
//!     match event {
//!         Event::Message(data) => println!("{}", String::from_utf8_lossy(&data)),
//!         Event::Paired => { /* persist conn.room() again */ }
//!         Event::State(State::Closed(_)) => break,
//!         _ => {}
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! The driver never stops reading the socket: incoming events are buffered
//! without bound (like a browser WebSocket), so an application that is slow
//! to call [`Connection::next`] costs memory but can never stall the
//! connection. Outgoing data is bounded by the protocol's window and the
//! outbox limit, beyond which [`Connection::send`] waits.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{CloseReason, Event, Output, Peer, Room, SendError, State, wire};

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type Connecting = Pin<Box<dyn Future<Output = Option<Ws>> + Send>>;
type Reply = oneshot::Sender<Result<u64, SendError>>;

/// Bytes written before each flush, at most.
const BATCH: usize = 256 << 10;
/// How long a closing socket may take to flush what is queued before it is dropped.
const FLUSH_GRACE: Duration = Duration::from_secs(30);

/// Connects to the room's relay and keeps the room connected, reconnecting
/// as needed, until the connection is left, closed or dropped. Must be
/// called within a Tokio runtime. A room that is closing carries on ending.
pub fn connect(room: Room) -> Connection {
    connect_with_token(room, None)
}

/// [`connect`], for a relay that requires a token (SPEC §4.1). On
/// [`Event::Unauthorized`], get another and call [`Connection::set_token`].
pub fn connect_with_token(room: Room, token: Option<String>) -> Connection {
    let shared = Arc::new(Mutex::new(room.clone()));
    let closing = Arc::new(AtomicBool::new(room.is_closing()));
    let (commands, rx) = mpsc::channel(64);
    let (control, controls) = mpsc::unbounded_channel();
    let (tx, events) = mpsc::unbounded_channel();
    tokio::spawn(drive(Peer::with_token(room, token, 0), rx, controls, tx, shared.clone()));
    Connection { sender: Sender { commands, closing }, control, events, room: shared }
}

/// A live room connection. Dropping it leaves the room.
pub struct Connection {
    sender: Sender,
    control: mpsc::UnboundedSender<Control>,
    events: mpsc::UnboundedReceiver<Event>,
    room: Arc<Mutex<Room>>,
}

impl Connection {
    /// The next event; `None` once the connection has stopped.
    pub async fn next(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Queues a message for the peer and returns its id (see [`Peer::send`]).
    /// Waits while too much is unacknowledged.
    pub async fn send(&self, data: impl Into<Vec<u8>>) -> Result<u64, SendError> {
        self.sender.send(data).await
    }

    /// A handle for sending from other tasks.
    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }

    /// The room as it currently stands; persist it after [`Event::Paired`]
    /// and after [`Connection::close`].
    pub fn room(&self) -> Room {
        self.room.lock().unwrap().clone()
    }

    /// Ends the room for both peers (see [`Peer::close`]): the connection
    /// keeps going until the peer has been told, then reports
    /// [`CloseReason::Closed`]. The room now says it is closing; persist it,
    /// so that reconnecting later finishes the job if this does not.
    pub fn close(&self) {
        self.sender.closing.store(true, Ordering::Relaxed);
        self.room.lock().unwrap().set_closing();
        let _ = self.control.send(Control::Close);
    }

    /// Replaces the relay token (see [`Peer::set_token`]).
    pub fn set_token(&self, token: Option<String>) {
        let _ = self.control.send(Control::Token(token));
    }

    /// Disconnects, keeping the room for later.
    pub async fn leave(mut self) {
        let _ = self.control.send(Control::Leave);
        while self.events.recv().await.is_some() {}
    }
}

#[derive(Clone)]
pub struct Sender {
    commands: mpsc::Sender<(Vec<u8>, Reply)>,
    /// Set by [`Connection::close`]: nothing is sent after that.
    closing: Arc<AtomicBool>,
}

impl Sender {
    pub async fn send(&self, data: impl Into<Vec<u8>>) -> Result<u64, SendError> {
        if self.closing.load(Ordering::Relaxed) {
            return Err(SendError::Closed);
        }
        let (reply, result) = oneshot::channel();
        if self.commands.send((data.into(), reply)).await.is_err() {
            return Err(SendError::Closed);
        }
        result.await.unwrap_or(Err(SendError::Closed))
    }
}

/// How the application stops the connection. Dropping it leaves.
enum Control {
    Close,
    Token(Option<String>),
    Leave,
}

async fn drive(
    mut peer: Peer,
    mut commands: mpsc::Receiver<(Vec<u8>, Reply)>,
    mut control: mpsc::UnboundedReceiver<Control>,
    events: mpsc::UnboundedSender<Event>,
    room: Arc<Mutex<Room>>,
) {
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u64;
    let mut socket: Option<Socket> = None;
    let mut connecting: Option<Connecting> = None;
    let mut flushing: Option<tokio::task::JoinHandle<()>> = None;
    // A send waiting for room in the outbox.
    let mut waiting: Option<(Vec<u8>, Reply)> = None;
    let mut controlled = true;
    // close() was called: sends issued before it go first.
    let mut closing = false;

    loop {
        if let Some((data, reply)) = waiting.take() {
            match peer.send(&data) {
                Err(SendError::Full) => waiting = Some((data, reply)),
                result => {
                    let _ = reply.send(result);
                }
            }
        }
        if closing && waiting.is_none() {
            match commands.try_recv() {
                Ok(command) => waiting = Some(command),
                Err(_) => {
                    closing = false;
                    peer.close();
                    let mut shared = room.lock().unwrap();
                    *shared = peer.room().clone();
                }
            }
            if waiting.is_some() {
                continue;
            }
        }
        while let Some(output) = peer.poll_output() {
            match output {
                Output::Connect(url) => connecting = Some(Box::pin(open(url))),
                Output::Send(frame) => {
                    if let Some(socket) = &socket {
                        let _ = socket.data.send(Message::binary(frame));
                    }
                }
                Output::Ping => {
                    if let Some(socket) = &socket {
                        let _ = socket.control.send(Message::text(wire::PING));
                    }
                }
                Output::Disconnect => {
                    connecting = None;
                    if let Some(socket) = socket.take() {
                        flushing = Some(socket.close());
                    }
                }
                Output::Event(event) => {
                    if event == Event::Paired {
                        let mut shared = room.lock().unwrap();
                        let closing = shared.is_closing(); // close() may have marked it already
                        *shared = peer.room().clone();
                        if closing {
                            shared.set_closing();
                        }
                    }
                    if events.send(event).is_err() {
                        peer.leave(); // nobody is listening any more
                    }
                }
            }
        }
        if let State::Closed(reason) = peer.state() {
            // Make sure the last word (the CLOSE, or our acknowledgement of
            // the peer's) leaves before returning.
            if reason != CloseReason::Left
                && let Some(flushing) = flushing
            {
                let _ = flushing.await;
            }
            return;
        }

        let deadline = peer.poll_timeout().map(|t| start + Duration::from_millis(t));
        tokio::select! {
            // Application requests and timers first: sends issued before a
            // close go before it, and pings are never starved by a busy
            // socket. Sends cannot starve reading either: the window and the
            // outbox limit park them until acknowledgements are read.
            biased;
            Some((data, reply)) = commands.recv(), if waiting.is_none() => {
                waiting = Some((data, reply)); // tried at the top of the loop
            }
            control = control.recv(), if controlled => match control {
                // Takes effect once the sends issued before it are queued.
                Some(Control::Close) => closing = true,
                Some(Control::Token(token)) => peer.set_token(now(), token),
                Some(Control::Leave) | None => {
                    controlled = false;
                    peer.leave();
                }
            },
            () = async { sleep_until(deadline.unwrap()).await }, if deadline.is_some() => {
                peer.handle_timeout(now());
            }
            ws = async { connecting.as_mut().unwrap().await }, if connecting.is_some() => {
                connecting = None;
                match ws {
                    Some(ws) => {
                        socket = Some(Socket::new(ws));
                        peer.handle_open(now());
                    }
                    None => peer.handle_close(now(), None),
                }
            }
            msg = async { socket.as_mut().unwrap().stream.next().await }, if socket.is_some() => match msg {
                Some(Ok(Message::Binary(frame))) => peer.handle_binary(now(), &frame),
                Some(Ok(Message::Text(text))) => peer.handle_text(now(), &text),
                Some(Ok(Message::Close(frame))) => {
                    socket.take().unwrap().close();
                    peer.handle_close(now(), frame.map(|f| f.code.into()));
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => {
                    socket.take().unwrap().close();
                    peer.handle_close(now(), None);
                }
            },
        }
    }
}

/// An open WebSocket: the driver reads it, a separate task writes it, so a
/// slow write never holds up reading.
struct Socket {
    stream: SplitStream<Ws>,
    data: mpsc::UnboundedSender<Message>,
    /// Keepalives, written ahead of queued data.
    control: mpsc::UnboundedSender<Message>,
    writer: tokio::task::JoinHandle<()>,
}

impl Socket {
    fn new(ws: Ws) -> Socket {
        let (sink, stream) = ws.split();
        let (data, data_rx) = mpsc::unbounded_channel();
        let (control, control_rx) = mpsc::unbounded_channel();
        let writer = tokio::spawn(write(sink, control_rx, data_rx));
        Socket { stream, data, control, writer }
    }

    /// Closes with 1000 once everything queued is written, abandoning a
    /// socket that does not drain in time. The returned task finishes then.
    fn close(self) -> tokio::task::JoinHandle<()> {
        let Socket { stream, data, control, mut writer } = self;
        drop((data, control));
        tokio::spawn(async move {
            if timeout(FLUSH_GRACE, &mut writer).await.is_err() {
                writer.abort();
            }
            drop(stream);
        })
    }
}

async fn write(
    mut sink: SplitSink<Ws, Message>,
    mut control: mpsc::UnboundedReceiver<Message>,
    mut data: mpsc::UnboundedReceiver<Message>,
) {
    loop {
        let first = tokio::select! {
            biased;
            Some(msg) = control.recv() => msg,
            msg = data.recv() => match msg {
                Some(msg) => msg,
                None => break,
            },
        };
        // Write what is ready, keepalives first, then flush once.
        let mut written = 0;
        let mut next = Some(first);
        while let Some(msg) = next {
            written += msg.len();
            if sink.feed(msg).await.is_err() {
                return;
            }
            next = control.try_recv().ok().or_else(|| (written < BATCH).then(|| data.try_recv().ok()).flatten());
        }
        if sink.flush().await.is_err() {
            return;
        }
    }
    let _ = sink.send(Message::Close(Some(CloseFrame { code: CloseCode::Normal, reason: "".into() }))).await;
}

async fn open(url: String) -> Option<Ws> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(wire::MAX_FRAME_LEN))
        .max_frame_size(Some(wire::MAX_FRAME_LEN));
    let (ws, _) = tokio_tungstenite::connect_async_with_config(url, Some(config), true).await.ok()?;
    Some(ws)
}
