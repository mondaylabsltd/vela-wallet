//! Frame layout, protocol constants and close codes (SPEC §5).
//!
//! Every binary WebSocket message is one frame: a type byte followed by a
//! body. Types below `0x80` are *peer frames*: the relay forwards them,
//! byte for byte, to the other seat and never looks past the type byte.
//! Types from `0x80` up are *relay frames*, exchanged with the relay itself.
//! Keepalive uses the text messages `ping` and `pong`.

use std::time::Duration;

/// Peer → peer: Noise handshake message 1 (joiner → inviter).
pub const INIT: u8 = 0x01;
/// Peer → peer: Noise handshake message 2 (inviter → joiner).
pub const REPLY: u8 = 0x02;
/// Peer → peer: Noise transport message.
pub const MESSAGE: u8 = 0x03;
/// Relay → peer: `nonce[32]`, the challenge for `JOIN`. Always the first frame.
pub const HELLO: u8 = 0x80;
/// Peer → relay: `role[1] signature[64]`.
pub const JOIN: u8 = 0x81;
/// Relay → peer: `present[1]`, whether a partner connection is in the other seat.
pub const PRESENCE: u8 = 0x82;

pub const PING: &str = "ping";
pub const PONG: &str = "pong";

pub const NONCE_LEN: usize = 32;
pub const HELLO_LEN: usize = 1 + NONCE_LEN;
pub const JOIN_LEN: usize = 1 + 1 + 64;
pub const PRESENCE_LEN: usize = 2;

/// Largest frame: a type byte plus the largest Noise message.
pub const MAX_FRAME_LEN: usize = 1 + crate::noise::MAX_MESSAGE_LEN;
/// Largest application message (it travels in as many frames as needed).
pub const MAX_MESSAGE_LEN: usize = 16 << 20;
/// Bytes a sender keeps in flight, sent but not acknowledged, at most (a
/// single message may overrun it). This, not the relay, bounds what is
/// buffered between two peers.
pub const WINDOW: usize = 4 << 20;

/// Domain separator for the `JOIN` signature.
pub const JOIN_CONTEXT: &[u8] = b"peerconnect/v1 join";

/// A peer sends `ping` this often, whatever else it sends.
pub const PING_INTERVAL: Duration = Duration::from_secs(25);
/// A peer that has received nothing for this long treats the connection as dead.
pub const RECEIVE_TIMEOUT: Duration = Duration::from_secs(60);
/// A relay may close a connection that has sent no `ping` for this long.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(90);
/// A relay closes a connection that has not joined within this time.
pub const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
/// A joiner restarts the connection if its handshake is not answered in time.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// WebSocket close codes used by the protocol.
pub mod close {
    pub const NORMAL: u16 = 1000;
    pub const GOING_AWAY: u16 = 1001;
    /// Malformed or unexpected frame.
    pub const PROTOCOL_ERROR: u16 = 1002;
    /// Frame larger than [`super::MAX_FRAME_LEN`].
    pub const TOO_BIG: u16 = 1009;
    /// `JOIN` signature did not verify.
    pub const UNAUTHORIZED: u16 = 4401;
    /// No `JOIN` within the join timeout, or idle for too long.
    pub const TIMEOUT: u16 = 4408;
    /// The relay requires a token, and this connection's is missing,
    /// invalid, expired or revoked (SPEC §4.1).
    pub const TOKEN: u16 = 4403;
    /// The relay requires tokens, and this one connects too often.
    pub const RATE_LIMITED: u16 = 4429;
    /// A newer connection took this seat.
    pub const REPLACED: u16 = 4409;
}

/// The two seats of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Created the room and handed out the invite. Noise responder.
    Inviter = 0,
    /// Joined with the invite. Noise initiator.
    Joiner = 1,
}

impl Role {
    pub fn from_byte(b: u8) -> Option<Role> {
        match b {
            0 => Some(Role::Inviter),
            1 => Some(Role::Joiner),
            _ => None,
        }
    }

    pub fn other(self) -> Role {
        match self {
            Role::Inviter => Role::Joiner,
            Role::Joiner => Role::Inviter,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

pub fn hello(nonce: &[u8; NONCE_LEN]) -> [u8; HELLO_LEN] {
    let mut f = [HELLO; HELLO_LEN];
    f[1..].copy_from_slice(nonce);
    f
}

pub fn presence(present: bool) -> [u8; PRESENCE_LEN] {
    [PRESENCE, present as u8]
}

/// The message a peer signs to answer a `HELLO`.
pub fn join_message(nonce: &[u8; NONCE_LEN], role: Role) -> Vec<u8> {
    [JOIN_CONTEXT, nonce, &[role as u8]].concat()
}

pub fn is_peer_frame(frame: &[u8]) -> bool {
    frame.first().is_some_and(|&t| t < 0x80)
}
