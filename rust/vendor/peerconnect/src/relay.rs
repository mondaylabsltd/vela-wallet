//! The relay's decisions (SPEC §5), shared by every relay implementation.
//!
//! A relay keeps, per room, at most one connection in each of the two seats.
//! Everything it does with a single message is decided here; seating,
//! eviction and forwarding are left to the runtime, which knows how its
//! connections are stored.

use crate::room::RoomId;
use crate::wire::{self, Role, close};

/// A WebSocket message as received by the relay.
#[derive(Debug, Clone, Copy)]
pub enum Incoming<'a> {
    Text(&'a str),
    Binary(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Reply with the text message `pong`.
    Pong,
    /// The connection proved access to the room: seat it in `Role`, closing
    /// any connection already there with [`close::REPLACED`].
    Join(Role),
    /// Forward the message unchanged to the connection in the other seat,
    /// or drop it if the seat is empty.
    Forward,
    /// Close the connection with this code.
    Close(u16),
}

/// A connection's position in the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conn {
    /// Sent `HELLO` with this nonce; waiting for `JOIN`.
    Pending([u8; wire::NONCE_LEN]),
    Seated(Role),
}

impl Conn {
    /// A new connection and the `HELLO` frame to send it first.
    pub fn new() -> (Conn, [u8; wire::HELLO_LEN]) {
        let nonce = crate::random();
        (Conn::Pending(nonce), wire::hello(&nonce))
    }

    /// Decides what to do with a message from this connection.
    pub fn receive(&self, room: &RoomId, message: Incoming<'_>) -> Action {
        match (self, message) {
            (_, Incoming::Text(text)) if text == wire::PING => Action::Pong,
            (_, Incoming::Binary(frame)) if frame.len() > wire::MAX_FRAME_LEN => Action::Close(close::TOO_BIG),
            (Conn::Pending(nonce), Incoming::Binary(frame)) => match room.verify_join(nonce, frame) {
                Ok(role) => Action::Join(role),
                Err(code) => Action::Close(code),
            },
            (Conn::Seated(_), Incoming::Binary(frame)) if wire::is_peer_frame(frame) => Action::Forward,
            _ => Action::Close(close::PROTOCOL_ERROR),
        }
    }
}

/// Extracts the room from a request path ending in `/v1/<room id>`. Any
/// prefix is allowed, so a relay can be mounted below a path.
pub fn room_from_path(path: &str) -> Option<RoomId> {
    let path = path.split('?').next()?;
    let (rest, id) = path.rsplit_once('/')?;
    rest.ends_with("/v1").then(|| id.parse().ok())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Room;

    #[test]
    fn decides_like_the_spec() {
        let room = Room::create("wss://r.example").unwrap();
        let id = room.id();
        let (pending, hello) = Conn::new();
        assert_eq!(hello[0], wire::HELLO);
        let Conn::Pending(nonce) = pending else { unreachable!() };

        let join = room.join_frame(crate::room::Rendezvous::Invite, &nonce);
        assert_eq!(pending.receive(&id, Incoming::Binary(&join)), Action::Join(Role::Inviter));
        assert_eq!(pending.receive(&id, Incoming::Text("ping")), Action::Pong);
        assert_eq!(pending.receive(&id, Incoming::Binary(&[wire::MESSAGE, 1])), Action::Close(close::PROTOCOL_ERROR));
        assert_eq!(
            pending.receive(&id, Incoming::Binary(&room.join_frame(crate::room::Rendezvous::Invite, &[0; 32]))),
            Action::Close(close::UNAUTHORIZED)
        );

        let seated = Conn::Seated(Role::Joiner);
        assert_eq!(seated.receive(&id, Incoming::Binary(&[wire::MESSAGE, 1, 2])), Action::Forward);
        assert_eq!(seated.receive(&id, Incoming::Binary(&[0x7f])), Action::Forward);
        assert_eq!(seated.receive(&id, Incoming::Binary(&join)), Action::Close(close::PROTOCOL_ERROR));
        assert_eq!(seated.receive(&id, Incoming::Binary(&[])), Action::Close(close::PROTOCOL_ERROR));
        assert_eq!(seated.receive(&id, Incoming::Text("hi")), Action::Close(close::PROTOCOL_ERROR));
        let big = vec![wire::MESSAGE; wire::MAX_FRAME_LEN + 1];
        assert_eq!(seated.receive(&id, Incoming::Binary(&big)), Action::Close(close::TOO_BIG));
    }

    #[test]
    fn parses_paths() {
        let id = Room::create("wss://r.example").unwrap().id();
        assert_eq!(room_from_path(&format!("/v1/{id}")), Some(id));
        assert_eq!(room_from_path(&format!("/relay/v1/{id}?x=1")), Some(id));
        assert_eq!(room_from_path(&format!("/v2/{id}")), None);
        assert_eq!(room_from_path(&format!("/v1/{id}/")), None);
        assert_eq!(room_from_path("/v1/abc"), None);
        assert_eq!(room_from_path("/"), None);
    }
}
