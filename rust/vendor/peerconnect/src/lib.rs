//! PeerConnect: two peers, one room, end-to-end encryption over an untrusted,
//! stateless WebSocket relay.
//!
//! This crate is the protocol, without I/O:
//!
//! - [`Room`] and [`Invite`]: room credentials, created by one peer and
//!   accepted by the other, persisted by both.
//! - [`Peer`]: the peer state machine. Drive it with any WebSocket
//!   implementation, or enable the `client` feature for a ready-made Tokio
//!   driver ([`client`]).
//! - [`RoomId::verify_join`] and [`wire`]: what a relay needs; with the
//!   `tokens` feature, [`token`] for a relay that requires credentials.
//!
//! The protocol is specified in `SPEC.md` at the repository root.

mod base64;
mod noise;
mod peer;
pub mod relay;
mod room;
#[cfg(feature = "tokens")]
pub mod token;
#[cfg(feature = "tokens")]
pub mod usage;
pub mod wire;

#[cfg(test)]
mod vectors;

#[cfg(feature = "client")]
pub mod client;

pub use peer::{CloseReason, Event, OUTBOX_LIMIT, Output, Peer, SendError, State};
pub use room::{Invite, Room, RoomId};
pub use wire::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Not a `ws://` or `wss://` URL without query or fragment.
    InvalidRelayUrl,
    InvalidInvite,
    InvalidRoomId,
    /// A persisted room record that does not parse.
    InvalidRecord,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::InvalidRelayUrl => "invalid relay URL",
            Error::InvalidInvite => "invalid invite",
            Error::InvalidRoomId => "invalid room id",
            Error::InvalidRecord => "invalid room record",
        })
    }
}

impl std::error::Error for Error {}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the system random number generator is available");
    bytes
}
