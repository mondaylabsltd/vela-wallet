//! PeerConnect's sans-I/O peer over UniFFI (native multisig M4): the rooms an
//! outside owner's wallet and Vela meet in (`multisig_outside`'s `peer_*`
//! operations). The protocol — the Noise handshake, the relay's JOIN, the
//! ordered, acknowledged, resumable messages — is PeerConnect's own crate
//! (`rust/vendor/peerconnect`), the same Rust the web runs as wasm and the
//! desktop links directly; a phone supplies only the WebSocket and the
//! clock, as the desktop's `multisig/peer.rs` does. Nothing here decides
//! what a message means: that is the core's (`multisig_outside`).
//!
//! ```text
//! peer = PeerConnectPeer(record, awaiting_token, now_ms)
//! loop { output = peer.poll_output() … ; peer.handle_*(now_ms, …) ; at peer.poll_timeout() → handle_timeout }
//! ```

use std::sync::Mutex;

use peerconnect::{CloseReason, Event, Invite, Output, Peer, Room, State};

use crate::CoreError;

/// A room's record (`Room`'s string form, kept to reopen it), and — for a
/// room just created — the invite to hand the other peer.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PeerRoomRecord {
    pub record: String,
    pub invite: Option<String>,
    pub relay: String,
}

/// A new room on `relay` (`ws://` or `wss://`): this side is the inviter.
#[uniffi::export]
pub fn peerconnect_create_room(relay: String) -> Result<PeerRoomRecord, CoreError> {
    let room = Room::create(&relay).map_err(|e| CoreError::Internal(format!("room: {e}")))?;
    Ok(PeerRoomRecord {
        record: room.to_string(),
        invite: room.invite().map(|invite| invite.to_string()),
        relay: room.relay().to_owned(),
    })
}

/// Join the room an invite names: this side is the joiner. The record must
/// be kept before connecting (joining again yields another identity).
#[uniffi::export]
pub fn peerconnect_join_room(invite: String) -> Result<PeerRoomRecord, CoreError> {
    let invite: Invite = invite
        .parse()
        .map_err(|e| CoreError::Internal(format!("invite: {e}")))?;
    let room = Room::join(&invite);
    Ok(PeerRoomRecord {
        record: room.to_string(),
        invite: None,
        relay: room.relay().to_owned(),
    })
}

/// A kept room record, read again (its relay; whether it still reads).
#[uniffi::export]
pub fn peerconnect_room_relay(record: String) -> Result<String, CoreError> {
    let room: Room = record
        .parse()
        .map_err(|e| CoreError::Internal(format!("record: {e}")))?;
    Ok(room.relay().to_owned())
}

/// What the peer asks of its driver, one at a time.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum PeerOutput {
    /// Open a WebSocket to `url` (any previous one is already closed).
    Connect { url: String },
    /// Send a binary frame.
    Send { bytes: Vec<u8> },
    /// Send the text frame `ping`.
    Ping,
    /// Close the WebSocket (1000) after flushing; report nothing more of it.
    Disconnect,
    /// The connection's state: `offline`, `waiting`, `connected`, or
    /// `closed` with `reason` (`left`, `closed`, `closed-by-peer`).
    State {
        state: String,
        reason: Option<String>,
    },
    /// A message from the other peer, authenticated, in order.
    Message { bytes: Vec<u8> },
    /// The other peer has every message with an id below `next`.
    Delivered { next: u64 },
    /// Pairing is complete: `record` reopens the room now (keep it).
    Paired { record: String },
    /// Another connection took this seat (the room opened elsewhere).
    Replaced,
    /// The relay wants a token (refused ours, or we have none).
    Unauthorized,
}

fn state_words(state: State) -> (String, Option<String>) {
    match state {
        State::Offline => ("offline".into(), None),
        State::Waiting => ("waiting".into(), None),
        State::Connected => ("connected".into(), None),
        State::Closed(reason) => (
            "closed".into(),
            Some(
                match reason {
                    CloseReason::Left => "left",
                    CloseReason::Closed => "closed",
                    CloseReason::ClosedByPeer => "closed-by-peer",
                }
                .into(),
            ),
        ),
    }
}

/// One room's peer state machine. Time is the driver's monotonic clock in ms.
#[derive(uniffi::Object)]
pub struct PeerConnectPeer(Mutex<Peer>);

#[uniffi::export]
impl PeerConnectPeer {
    /// The peer for a kept room `record`; `awaiting_token`: the relay admits
    /// only a token-holder, so it stays offline until [`Self::set_token`].
    #[uniffi::constructor]
    pub fn new(record: String, awaiting_token: bool, now_ms: u64) -> Result<Self, CoreError> {
        let room: Room = record
            .parse()
            .map_err(|e| CoreError::Internal(format!("record: {e}")))?;
        let peer = if awaiting_token {
            Peer::awaiting_token(room, now_ms)
        } else {
            Peer::new(room, now_ms)
        };
        Ok(Self(Mutex::new(peer)))
    }

    /// The next thing to do, or `None` when nothing is asked.
    pub fn poll_output(&self) -> Option<PeerOutput> {
        let mut peer = crate::onboarding_bridge::locked(&self.0).ok()?;
        let output = peer.poll_output()?;
        Some(match output {
            Output::Connect(url) => PeerOutput::Connect { url },
            Output::Send(bytes) => PeerOutput::Send { bytes },
            Output::Ping => PeerOutput::Ping,
            Output::Disconnect => PeerOutput::Disconnect,
            Output::Event(Event::State(state)) => {
                let (state, reason) = state_words(state);
                PeerOutput::State { state, reason }
            }
            Output::Event(Event::Message(bytes)) => PeerOutput::Message { bytes },
            Output::Event(Event::Delivered(next)) => PeerOutput::Delivered { next },
            Output::Event(Event::Paired) => PeerOutput::Paired {
                record: peer.room().to_string(),
            },
            Output::Event(Event::Replaced) => PeerOutput::Replaced,
            Output::Event(Event::Unauthorized) => PeerOutput::Unauthorized,
        })
    }

    /// When [`Self::handle_timeout`] is next due (ms), if ever.
    pub fn poll_timeout(&self) -> Option<u64> {
        crate::onboarding_bridge::locked(&self.0)
            .ok()?
            .poll_timeout()
    }

    pub fn handle_timeout(&self, now_ms: u64) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.handle_timeout(now_ms);
        }
    }

    /// The socket asked for opened.
    pub fn handle_open(&self, now_ms: u64) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.handle_open(now_ms);
        }
    }

    /// The socket closed or failed (with its close code when one came).
    pub fn handle_close(&self, now_ms: u64, code: Option<u16>) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.handle_close(now_ms, code);
        }
    }

    pub fn handle_text(&self, now_ms: u64, text: String) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.handle_text(now_ms, &text);
        }
    }

    pub fn handle_binary(&self, now_ms: u64, frame: Vec<u8>) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.handle_binary(now_ms, &frame);
        }
    }

    /// Queue a message for the other peer: its id, or why not.
    pub fn send(&self, bytes: Vec<u8>) -> Result<u64, CoreError> {
        crate::onboarding_bridge::locked(&self.0)?
            .send(&bytes)
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub fn set_token(&self, now_ms: u64, token: Option<String>) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.set_token(now_ms, token);
        }
    }

    /// Leave (the room stays valid; reopen it from its record).
    pub fn leave(&self) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.leave();
        }
    }

    /// End the room for both peers (not `close`: a UniFFI object already has one, freeing it).
    pub fn end(&self) {
        if let Ok(mut peer) = crate::onboarding_bridge::locked(&self.0) {
            peer.close();
        }
    }

    /// The room's record as it stands (paired or not).
    pub fn record(&self) -> String {
        crate::onboarding_bridge::locked(&self.0)
            .map(|peer| peer.room().to_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A room created reads back; its invite joins; both peers ask to
    /// connect to the relay first.
    #[test]
    fn a_room_is_created_joined_and_asks_for_the_relay() {
        let created = peerconnect_create_room("ws://127.0.0.1:9".to_owned()).unwrap();
        assert!(created.invite.is_some());
        assert_eq!(created.relay, "ws://127.0.0.1:9");
        assert_eq!(
            peerconnect_room_relay(created.record.clone()).unwrap(),
            "ws://127.0.0.1:9"
        );
        let joined = peerconnect_join_room(created.invite.clone().unwrap()).unwrap();
        assert!(joined.invite.is_none());

        let inviter = PeerConnectPeer::new(created.record, false, 0).unwrap();
        let joiner = PeerConnectPeer::new(joined.record, false, 0).unwrap();
        for peer in [&inviter, &joiner] {
            let first: Vec<PeerOutput> = std::iter::from_fn(|| peer.poll_output()).collect();
            assert!(
                first.iter().any(|o| matches!(o, PeerOutput::Connect { url } if url.starts_with("ws://127.0.0.1:9/v1/"))),
                "{first:?}"
            );
        }
        // A message queued offline is kept for the session.
        assert!(inviter.send(b"hello".to_vec()).is_ok());
        assert!(peerconnect_join_room("not an invite".to_owned()).is_err());
        assert!(PeerConnectPeer::new("nonsense".to_owned(), false, 0).is_err());
    }

    /// A peer that waits for a token connects to nothing until it has one.
    #[test]
    fn a_peer_awaiting_a_token_stays_offline() {
        let created = peerconnect_create_room("wss://relay.example".to_owned()).unwrap();
        let peer = PeerConnectPeer::new(created.record, true, 0).unwrap();
        let first: Vec<PeerOutput> = std::iter::from_fn(|| peer.poll_output()).collect();
        assert!(
            !first
                .iter()
                .any(|o| matches!(o, PeerOutput::Connect { .. })),
            "{first:?}"
        );
        peer.set_token(1, Some("t".to_owned()));
        let next: Vec<PeerOutput> = std::iter::from_fn(|| peer.poll_output()).collect();
        assert!(
            next.iter().any(|o| matches!(o, PeerOutput::Connect { .. })),
            "{next:?}"
        );
    }
}
