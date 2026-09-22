//! The native host of the Clear Signer relay (`vela-relay/1`).
//!
//! `GET /v1/rooms/{room}?role=…` upgrades to a WebSocket; `GET /healthz` answers
//! `ok`. The rules — ids, roles, pairing, limits, close codes — are
//! [`vela_relay`]'s; this crate only moves bytes and keeps time.
//!
//! Shape: each room is a task that owns its [`Room`] and the two ends' outboxes,
//! so everything that happens to a room happens in one order — a `left` can never
//! overtake the frames sent before it, nor a `joined` the `left` before it. Each
//! end is a reader (this connection's frames → its room) and a writer (its outbox
//! and pings → the socket). Rooms live in memory and are forgotten when empty.
//!
//! Frame contents are never logged; only room counts and lifetimes are.

use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{sleep_until, timeout, Instant};
use tokio_tungstenite::tungstenite::error::CapacityError;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Role as WsRole, WebSocketConfig};
use tokio_tungstenite::tungstenite::{Error as WsError, Message, Utf8Bytes};
use tokio_tungstenite::WebSocketStream;
use vela_relay::{Action, Close, ConnId, Millis, Role, Room, Target};

/// What the WebSocket library will buffer for one message. Deliberately above
/// [`vela_relay::MAX_FRAME_BYTES`]: a frame a little over the limit is read whole,
/// so the room's verdict (1009) arrives on a clean close. Past this cap the
/// library refuses to buffer at all; the host still closes 1009, but stops
/// reading the rest.
const READ_CAP: usize = 2 * vela_relay::MAX_FRAME_BYTES;
/// Frames queued for one end before its room waits for it to drain.
const OUTBOX: usize = 16;
/// Events queued for one room.
const ROOM_QUEUE: usize = 16;
/// How long a room waits for an end to take a frame. An end that stops reading
/// for this long is treated as idle, so one stuck socket cannot stall its room's
/// clock.
const STALL: Duration = Duration::from_secs(10);
/// How long a closed end has to answer the close frame before it is dropped.
const CLOSE_GRACE: Duration = Duration::from_secs(5);
/// How long a client has to send its request headers.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);

type Socket = WebSocketStream<TokioIo<Upgraded>>;

/// Serves the relay on `listener` until the future is dropped.
pub async fn serve(listener: TcpListener) {
    let relay = Arc::new(Relay::new());
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(err) => {
                // Out of file descriptors, most likely: back off, do not exit.
                eprintln!("vela-relay: accept failed: {err}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let relay = relay.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let relay = relay.clone();
                async move { Ok::<_, Infallible>(relay.respond(req)) }
            });
            // A failed connection is the client's business; nothing to log.
            let _ = http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEADER_TIMEOUT)
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await;
        });
    }
}

struct Relay {
    rooms: Mutex<HashMap<String, mpsc::Sender<Event>>>,
    next_conn: AtomicU64,
    epoch: Instant,
}

/// What an end tells its room.
enum Event {
    /// A new connection asks for `role`. `ack` answers once the room has taken it
    /// in (or refused it); dropped unanswered, the room was retiring — try again.
    Connect {
        conn: ConnId,
        role: Role,
        outbox: Outbox,
        ack: oneshot::Sender<()>,
    },
    /// A text or binary frame of `len` bytes. `msg` is `None` when it was too big
    /// to buffer.
    Frame {
        conn: ConnId,
        len: usize,
        msg: Option<Message>,
    },
    /// The connection closed or dropped.
    Leave { conn: ConnId },
}

/// How a room reaches one end.
#[derive(Clone)]
struct Outbox {
    frames: mpsc::Sender<Message>,
    close: Arc<watch::Sender<Option<Close>>>,
}

impl Outbox {
    /// Closes the end, ahead of anything still queued. The first reason wins.
    fn close(&self, close: Close) {
        self.close.send_if_modified(|current| {
            let first = current.is_none();
            if first {
                *current = Some(close);
            }
            first
        });
    }
}

impl Relay {
    fn new() -> Self {
        Relay {
            rooms: Mutex::new(HashMap::new()),
            next_conn: AtomicU64::new(1),
            epoch: Instant::now(),
        }
    }

    /// The rules' clock: milliseconds since start, monotonic.
    fn now(&self) -> Millis {
        Millis::try_from(self.epoch.elapsed().as_millis()).unwrap_or(Millis::MAX)
    }

    fn instant(&self, at: Millis) -> Instant {
        self.epoch + Duration::from_millis(at)
    }

    fn rooms(&self) -> MutexGuard<'_, HashMap<String, mpsc::Sender<Event>>> {
        // Nothing panics while holding the lock; a poisoned map is still sound.
        self.rooms
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn respond(self: &Arc<Self>, mut req: Request<Incoming>) -> Response<Full<Bytes>> {
        let (room, role) = match vela_relay::target(req.uri().path(), req.uri().query()) {
            Target::Healthz => return text(StatusCode::OK, "ok"),
            Target::NotFound => return text(StatusCode::NOT_FOUND, "not found"),
            Target::BadRequest(why) => return text(StatusCode::BAD_REQUEST, why),
            Target::Room { room, role } => (room.to_owned(), role),
        };
        if req.method() != Method::GET {
            return text(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
        }
        if !has_token(req.headers(), header::UPGRADE, "websocket") {
            let mut response = text(StatusCode::UPGRADE_REQUIRED, "websocket upgrade required");
            response
                .headers_mut()
                .insert(header::UPGRADE, HeaderValue::from_static("websocket"));
            return response;
        }
        let Some(accept) = accept_key(req.headers()) else {
            return text(StatusCode::BAD_REQUEST, "bad websocket handshake");
        };
        let upgrade = hyper::upgrade::on(&mut req);
        let relay = self.clone();
        tokio::spawn(async move {
            if let Ok(upgraded) = upgrade.await {
                let config = WebSocketConfig::default()
                    .max_message_size(Some(READ_CAP))
                    .max_frame_size(Some(READ_CAP));
                let socket = WebSocketStream::from_raw_socket(
                    TokioIo::new(upgraded),
                    WsRole::Server,
                    Some(config),
                )
                .await;
                relay.run_end(room, role, socket).await;
            }
        });
        let mut response = Response::new(Full::default());
        *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        let headers = response.headers_mut();
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
        headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
        headers.insert(header::SEC_WEBSOCKET_ACCEPT, accept);
        response
    }

    /// One end's life: join its room, pump its frames there, leave.
    async fn run_end(self: Arc<Self>, room: String, role: Role, socket: Socket) {
        let conn = self.next_conn.fetch_add(1, Ordering::Relaxed);
        let (frames, queued) = mpsc::channel(OUTBOX);
        let (close, mut closed) = watch::channel(None);
        let outbox = Outbox {
            frames,
            close: Arc::new(close),
        };
        let (sink, mut stream) = socket.split();
        let mut writer = tokio::spawn(write(sink, queued, closed.clone()));
        let events = self.enter(&room, role, conn, outbox.clone()).await;

        let mut grace: Option<Instant> = None;
        loop {
            let next = tokio::select! {
                next = stream.next() => next,
                // Closed by the room: the writer sends the close frame; give the
                // client a moment to answer it, then drop the connection.
                Ok(()) = closed.changed(), if grace.is_none() => {
                    grace = Some(Instant::now() + CLOSE_GRACE);
                    continue;
                }
                () = sleep_until(grace.unwrap_or_else(Instant::now)), if grace.is_some() => break,
            };
            match next {
                Some(Ok(msg @ (Message::Text(_) | Message::Binary(_)))) => {
                    let len = msg.len();
                    let event = Event::Frame {
                        conn,
                        len,
                        msg: Some(msg),
                    };
                    if events.send(event).await.is_err() {
                        break;
                    }
                }
                // The client is closing. Tell the room now; the next read sends
                // the library's reply and ends the stream.
                Some(Ok(Message::Close(_))) => {
                    let _ = events.send(Event::Leave { conn }).await;
                }
                // Pings are answered by the library; pongs need nothing.
                Some(Ok(_)) => {}
                Some(Err(WsError::Capacity(CapacityError::MessageTooLong { size, .. }))) => {
                    let _ = events
                        .send(Event::Frame {
                            conn,
                            len: size,
                            msg: None,
                        })
                        .await;
                    // Let the writer get the close frame out, then stop: the rest
                    // of the frame is not worth reading.
                    let _ = timeout(CLOSE_GRACE, &mut writer).await;
                    break;
                }
                Some(Err(_)) | None => break,
            }
        }
        let _ = events.send(Event::Leave { conn }).await;
        writer.abort();
    }

    /// Hands a new connection to its room, creating the room if need be.
    async fn enter(
        self: &Arc<Self>,
        room: &str,
        role: Role,
        conn: ConnId,
        outbox: Outbox,
    ) -> mpsc::Sender<Event> {
        loop {
            let events = self.room_events(room);
            let (ack, acked) = oneshot::channel();
            let connect = Event::Connect {
                conn,
                role,
                outbox: outbox.clone(),
                ack,
            };
            if events.send(connect).await.is_ok() && acked.await.is_ok() {
                return events;
            }
            // The room retired between the lookup and the send: a fresh one.
        }
    }

    fn room_events(self: &Arc<Self>, id: &str) -> mpsc::Sender<Event> {
        let mut rooms = self.rooms();
        if let Some(events) = rooms.get(id).filter(|events| !events.is_closed()) {
            return events.clone();
        }
        let (events, inbox) = mpsc::channel(ROOM_QUEUE);
        rooms.insert(id.to_owned(), events.clone());
        let open = rooms.len();
        drop(rooms);
        eprintln!("vela-relay: room opened (rooms open: {open})");
        let task = RoomTask {
            relay: self.clone(),
            id: id.to_owned(),
            room: Room::new(self.now()),
            ends: HashMap::new(),
        };
        tokio::spawn(task.run(inbox));
        events
    }
}

struct RoomTask {
    relay: Arc<Relay>,
    id: String,
    room: Room,
    ends: HashMap<ConnId, Outbox>,
}

enum Step {
    Event(Event),
    Tick,
}

impl RoomTask {
    async fn run(mut self, mut inbox: mpsc::Receiver<Event>) {
        loop {
            let deadline = self.room.next_deadline().map(|at| self.relay.instant(at));
            let step = match deadline {
                Some(at) => tokio::select! {
                    event = inbox.recv() => event.map(Step::Event),
                    () = sleep_until(at) => Some(Step::Tick),
                },
                None => inbox.recv().await.map(Step::Event),
            };
            let Some(step) = step else { return };
            let now = self.relay.now();
            let (actions, frame) = match step {
                Step::Tick => (self.room.on_tick(now), None),
                Step::Event(Event::Connect {
                    conn,
                    role,
                    outbox,
                    ack,
                }) => {
                    self.ends.insert(conn, outbox);
                    let actions = self.room.on_connect(role, conn, now);
                    let _ = ack.send(());
                    (actions, None)
                }
                Step::Event(Event::Frame { conn, len, msg }) => {
                    (self.room.on_frame(conn, len, now), msg)
                }
                Step::Event(Event::Leave { conn }) => (self.room.on_leave(conn, now), None),
            };
            self.apply(actions, frame).await;
            let room = &self.room;
            self.ends.retain(|conn, _| room.occupant(*conn).is_some());
            if self.room.is_empty() && self.retire(&mut inbox) {
                return;
            }
        }
    }

    async fn apply(&mut self, actions: Vec<Action>, mut frame: Option<Message>) {
        let mut queue = VecDeque::from(actions);
        while let Some(action) = queue.pop_front() {
            match action {
                Action::Forward { to } => {
                    if let Some(msg) = frame.take() {
                        self.deliver(to, msg, &mut queue).await;
                    }
                }
                Action::Send { to, control } => {
                    let msg = Message::Text(Utf8Bytes::from_static(control.text()));
                    self.deliver(to, msg, &mut queue).await;
                }
                Action::Close { conn, close } => {
                    if let Some(end) = self.ends.get(&conn) {
                        end.close(close);
                    }
                }
            }
        }
    }

    async fn deliver(&mut self, to: ConnId, msg: Message, queue: &mut VecDeque<Action>) {
        let Some(end) = self.ends.get(&to) else {
            return;
        };
        // An error means the end's writer is gone; its reader reports the leave.
        if timeout(STALL, end.frames.send(msg)).await.is_err() {
            queue.extend(self.room.on_leave(to, self.relay.now()));
            end.close(Close::Idle);
        }
    }

    /// Forgets an empty room — unless a connection is already on its way in.
    fn retire(&self, inbox: &mut mpsc::Receiver<Event>) -> bool {
        let mut rooms = self.relay.rooms();
        if !inbox.is_empty() {
            return false;
        }
        rooms.remove(&self.id);
        // Anything sent from here on fails, and a `Connect` that slipped in
        // before this line is dropped unanswered: both make the sender retry
        // against a fresh room.
        inbox.close();
        let open = rooms.len();
        drop(rooms);
        let lived = self.relay.now().saturating_sub(self.room.created_at()) / 1000;
        eprintln!("vela-relay: room closed after {lived}s (rooms open: {open})");
        true
    }
}

/// An end's writer: its outbox and a ping every 30 s, until the room closes it.
async fn write(
    mut sink: SplitSink<Socket, Message>,
    mut outbox: mpsc::Receiver<Message>,
    mut closed: watch::Receiver<Option<Close>>,
) {
    let period = Duration::from_millis(vela_relay::PING_INTERVAL_MS);
    let mut ping = tokio::time::interval_at(Instant::now() + period, period);
    {
        let pump = async {
            loop {
                let msg = tokio::select! {
                    msg = outbox.recv() => match msg {
                        Some(msg) => msg,
                        None => return,
                    },
                    _ = ping.tick() => Message::Ping(Bytes::new()),
                };
                if sink.send(msg).await.is_err() {
                    return;
                }
            }
        };
        tokio::select! {
            biased;
            _ = closed.wait_for(Option::is_some) => {}
            () = pump => return,
        }
    }
    let close = *closed.borrow();
    if let Some(close) = close {
        let frame = CloseFrame {
            code: CloseCode::from(close.code()),
            reason: Utf8Bytes::from_static(close.reason()),
        };
        let _ = timeout(CLOSE_GRACE, sink.send(Message::Close(Some(frame)))).await;
    }
}

fn text(status: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from_static(body.as_bytes())));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

/// Whether a comma-separated header carries `token` (case-insensitive).
fn has_token(headers: &HeaderMap, name: header::HeaderName, token: &str) -> bool {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|part| part.trim().eq_ignore_ascii_case(token))
}

/// `Sec-WebSocket-Accept` for a well-formed RFC 6455 request.
fn accept_key(headers: &HeaderMap) -> Option<HeaderValue> {
    if !has_token(headers, header::CONNECTION, "upgrade") {
        return None;
    }
    if headers.get(header::SEC_WEBSOCKET_VERSION)?.as_bytes() != b"13" {
        return None;
    }
    let key = headers.get(header::SEC_WEBSOCKET_KEY)?;
    HeaderValue::from_str(&derive_accept_key(key.as_bytes())).ok()
}
