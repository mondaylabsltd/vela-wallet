//! The Clear Signer relay (`vela-relay/1`) as a Cloudflare Worker.
//!
//! The Worker answers `/healthz` and refuses bad room URLs itself (HTTP 400 before
//! any upgrade), then hands each room URL to the room's Durable Object —
//! `idFromName(room)`, so both ends of a room meet in one object. The object
//! holds the sockets on the WebSocket **hibernation** API: between frames it
//! can be evicted from memory while the connections stay up, so all it keeps is
//! what each socket carries in its attachment ([`Tag`]). Every event rebuilds
//! the [`Room`] from those tags, lets the shared rules decide, and carries out
//! the [`Action`]s. The room's two clocks (its life and each end's idleness)
//! are one alarm set to [`Room::next_deadline`].
//!
//! The hibernation API answers pings itself, so this host sends none. Frame
//! contents are never logged.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use vela_relay::{Action, ConnId, Millis, Occupant, Role, Room, Target};
use worker::*;

/// The Durable Object namespace binding (`wrangler.toml`).
const ROOMS: &str = "ROOMS";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    match vela_relay::target(url.path(), url.query()) {
        Target::Healthz => Response::ok("ok"),
        Target::NotFound => Response::error("not found", 404),
        Target::BadRequest(why) => Response::error(why, 400),
        Target::Room { room, .. } => {
            if !wants_websocket(&req)? {
                let mut response = Response::error("websocket upgrade required", 426)?;
                response.headers_mut().set("Upgrade", "websocket")?;
                return Ok(response);
            }
            let stub = env.durable_object(ROOMS)?.id_from_name(room)?.get_stub()?;
            stub.fetch_with_request(req).await
        }
    }
}

fn wants_websocket(req: &Request) -> Result<bool> {
    Ok(req
        .headers()
        .get("Upgrade")?
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket")))
}

/// What a socket carries across hibernation: who it is in the room and the
/// room's clocks.
#[derive(Clone, Serialize, Deserialize)]
struct Tag {
    conn: ConnId,
    role: String,
    created_at: Millis,
    last_frame_at: Millis,
    /// The room already let this socket go (it closed it, or saw it leave):
    /// ignore anything more from it.
    closed: bool,
}

fn tag_of(ws: &WebSocket) -> Option<Tag> {
    ws.deserialize_attachment().ok().flatten()
}

/// One room.
#[durable_object]
pub struct RelayRoom {
    state: State,
}

impl DurableObject for RelayRoom {
    fn new(state: State, _env: Env) -> Self {
        RelayRoom { state }
    }

    /// A connection for this room (the Worker already checked the URL).
    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let Target::Room { role, .. } = vela_relay::target(url.path(), url.query()) else {
            return Response::error("bad request", 400);
        };
        let now = now();
        let conn = new_conn_id();
        let (mut room, mut sockets) = self.restore(now, None);
        let actions = room.on_connect(role, conn, now);

        let WebSocketPair { client, server } = WebSocketPair::new()?;
        if room.occupant(conn).is_some() {
            self.state.accept_web_socket(&server);
            server.serialize_attachment(Tag {
                conn,
                role: role.as_str().to_owned(),
                created_at: room.created_at(),
                last_frame_at: now,
                closed: false,
            })?;
        } else {
            // Refused (its role is taken): accepted only for `apply` to close
            // it with 4409 right away. Not on the hibernation API — a
            // hibernatable socket closed before its 101 has gone out never
            // delivers the close frame.
            server.accept()?;
        }
        sockets.insert(conn, server);
        apply(&actions, &sockets, None);
        self.schedule(&room).await?;
        Response::from_websocket(client)
    }

    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        let Some(tag) = tag_of(&ws).filter(|tag| !tag.closed) else {
            return Ok(());
        };
        let now = now();
        let (mut room, sockets) = self.restore(now, None);
        let len = match &message {
            WebSocketIncomingMessage::String(text) => text.len(),
            WebSocketIncomingMessage::Binary(bytes) => bytes.len(),
        };
        let actions = room.on_frame(tag.conn, len, now);
        apply(&actions, &sockets, Some(&message));
        if let Some(me) = room.occupant(tag.conn) {
            ws.serialize_attachment(Tag {
                last_frame_at: me.last_frame_at,
                ..tag
            })?;
        }
        // Activity only moves deadlines later, so the armed alarm stays early
        // enough; it re-arms itself when it fires. Only an emptied room needs
        // its alarm cleared.
        if room.is_empty() {
            self.schedule(&room).await?;
        }
        Ok(())
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // The runtime answers the client's close frame itself
        // (web_socket_auto_reply_to_close, compatibility date 2026-04-07+).
        self.leave(ws).await
    }

    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        self.leave(ws).await
    }

    async fn alarm(&self) -> Result<Response> {
        let now = now();
        let (mut room, sockets) = self.restore(now, None);
        let actions = room.on_tick(now);
        apply(&actions, &sockets, None);
        self.schedule(&room).await?;
        Response::ok("")
    }
}

impl RelayRoom {
    /// Rebuilds the room from its live sockets' tags — plus `leaving`, a socket
    /// whose close is being handled and which the runtime may no longer list.
    fn restore(
        &self,
        now: Millis,
        leaving: Option<&WebSocket>,
    ) -> (Room, HashMap<ConnId, WebSocket>) {
        let live = self
            .state
            .get_websockets()
            .into_iter()
            .filter(|ws| ws.as_ref().ready_state() <= web_sys::WebSocket::OPEN);
        let mut sockets = HashMap::new();
        let mut occupants = Vec::new();
        let mut created_at: Option<Millis> = None;
        for ws in live.chain(leaving.cloned()) {
            let Some(tag) = tag_of(&ws) else { continue };
            let Some(role) = Role::parse(&tag.role) else {
                continue;
            };
            if tag.closed || sockets.contains_key(&tag.conn) {
                continue;
            }
            created_at = Some(created_at.map_or(tag.created_at, |at| at.min(tag.created_at)));
            occupants.push(Occupant {
                conn: tag.conn,
                role,
                last_frame_at: tag.last_frame_at,
            });
            sockets.insert(tag.conn, ws);
        }
        (Room::restore(created_at.unwrap_or(now), occupants), sockets)
    }

    /// A socket closed or failed: its peer hears `left`.
    async fn leave(&self, ws: WebSocket) -> Result<()> {
        let Some(tag) = tag_of(&ws).filter(|tag| !tag.closed) else {
            return Ok(());
        };
        let now = now();
        let (mut room, sockets) = self.restore(now, Some(&ws));
        let actions = room.on_leave(tag.conn, now);
        apply(&actions, &sockets, None);
        // The socket is done; the tag may already be gone with it.
        let _ = ws.serialize_attachment(Tag {
            closed: true,
            ..tag
        });
        if room.is_empty() {
            self.schedule(&room).await?;
        }
        Ok(())
    }

    /// One alarm for all of the room's clocks; none for an empty room, which is
    /// thereby forgotten (it stores nothing else).
    async fn schedule(&self, room: &Room) -> Result<()> {
        let storage = self.state.storage();
        match room.next_deadline() {
            // An absolute time: `set_alarm(i64)` would read it as an offset.
            Some(at) => {
                let at = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(at as f64));
                storage.set_alarm(ScheduledTime::new(at)).await
            }
            None => storage.delete_alarm().await,
        }
    }
}

/// Carries out the rules' verdicts. Send errors are ignored: they mean the
/// socket is already closing, and its close event reports the leave.
fn apply(
    actions: &[Action],
    sockets: &HashMap<ConnId, WebSocket>,
    frame: Option<&WebSocketIncomingMessage>,
) {
    for action in actions {
        match *action {
            Action::Forward { to } => {
                if let (Some(ws), Some(frame)) = (sockets.get(&to), frame) {
                    let _ = match frame {
                        WebSocketIncomingMessage::String(text) => ws.send_with_str(text),
                        WebSocketIncomingMessage::Binary(bytes) => ws.send_with_bytes(bytes),
                    };
                }
            }
            Action::Send { to, control } => {
                if let Some(ws) = sockets.get(&to) {
                    let _ = ws.send_with_str(control.text());
                }
            }
            Action::Close { conn, close } => {
                if let Some(ws) = sockets.get(&conn) {
                    if let Some(tag) = tag_of(ws) {
                        let _ = ws.serialize_attachment(Tag {
                            closed: true,
                            ..tag
                        });
                    }
                    let _ = ws.close(Some(close.code()), Some(close.reason()));
                }
            }
        }
    }
}

fn now() -> Millis {
    Date::now().as_millis()
}

/// A random connection id below 2^53, so it survives the trip through a JS number.
fn new_conn_id() -> ConnId {
    (js_sys::Math::random() * 9_007_199_254_740_992.0) as ConnId
}
