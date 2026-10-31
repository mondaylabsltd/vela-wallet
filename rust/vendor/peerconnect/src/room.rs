//! Room credentials: the room secret, what is derived from it, the invite,
//! and the record a peer keeps to reconnect (SPEC §3, §4).

use std::fmt;
use std::str::FromStr;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::noise::hmac;
use crate::wire::{self, Role, close};
use crate::{Error, base64};

/// Identifies a room on a relay. It is the room's Ed25519 public key, so it
/// locates the room without granting access to it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId([u8; 32]);

impl RoomId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Relay side: checks a `JOIN` frame against the `HELLO` nonce and returns
    /// the seat it claims, or the close code to reject it with.
    pub fn verify_join(&self, nonce: &[u8; wire::NONCE_LEN], frame: &[u8]) -> Result<Role, u16> {
        if frame.len() != wire::JOIN_LEN || frame[0] != wire::JOIN {
            return Err(close::PROTOCOL_ERROR);
        }
        let role = Role::from_byte(frame[1]).ok_or(close::PROTOCOL_ERROR)?;
        let signature = Signature::from_bytes(frame[2..].try_into().unwrap());
        let key = VerifyingKey::from_bytes(&self.0).map_err(|_| close::UNAUTHORIZED)?;
        key.verify_strict(&wire::join_message(nonce, role), &signature).map_err(|_| close::UNAUTHORIZED)?;
        Ok(role)
    }
}

impl FromStr for RoomId {
    type Err = Error;

    /// Parses the base64url form, accepting only valid Ed25519 public keys.
    fn from_str(s: &str) -> Result<Self, Error> {
        let bytes = base64::decode_array(s).ok_or(Error::InvalidRoomId)?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| Error::InvalidRoomId)?;
        Ok(RoomId(bytes))
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&base64::encode(&self.0))
    }
}

impl fmt::Debug for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RoomId({self})")
    }
}

/// Everything derived from a room secret alone.
#[derive(Clone)]
struct Keys {
    secret: Zeroizing<[u8; 32]>,
    /// Signs `JOIN` at the invite ID.
    invite: SigningKey,
    psk: Zeroizing<[u8; 32]>,
}

impl Keys {
    fn derive(secret: [u8; 32]) -> Keys {
        let seed = Zeroizing::new(hmac(&secret, &[b"peerconnect/v1 room"]));
        Keys {
            invite: SigningKey::from_bytes(&seed),
            psk: Zeroizing::new(hmac(&secret, &[b"peerconnect/v1 psk"])),
            secret: Zeroizing::new(secret),
        }
    }
}

/// Where on the relay the two peers meet (SPEC §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rendezvous {
    /// The invite ID: derived from the room secret, known to every holder of
    /// the invite. Used only until the room is paired.
    Invite,
    /// The pair ID: derived from the room secret and the two identities,
    /// known only to the two peers. Used once paired.
    Pair,
}

/// An invitation to a room: `<relay URL>#<base64url(secret ‖ inviter key)>`.
///
/// The invite is a bearer credential until the room is paired: whoever holds
/// it first can pair with the inviter. It must travel over a channel that
/// keeps it confidential and authentic. After pairing it is useless.
#[derive(Clone)]
pub struct Invite {
    relay: String,
    secret: Zeroizing<[u8; 32]>,
    inviter: PublicKey,
}

impl Invite {
    pub fn relay(&self) -> &str {
        &self.relay
    }

    pub fn room_id(&self) -> RoomId {
        RoomId(Keys::derive(*self.secret).invite.verifying_key().to_bytes())
    }
}

impl FromStr for Invite {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let (relay, fragment) = s.trim().split_once('#').ok_or(Error::InvalidInvite)?;
        let bytes = Zeroizing::new(base64::decode_array::<64>(fragment).ok_or(Error::InvalidInvite)?);
        let inviter = PublicKey::from(<[u8; 32]>::try_from(&bytes[32..]).unwrap());
        if !usable(&inviter) {
            return Err(Error::InvalidInvite);
        }
        Ok(Invite { relay: relay_url(relay)?, secret: Zeroizing::new(bytes[..32].try_into().unwrap()), inviter })
    }
}

impl fmt::Display for Invite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = Zeroizing::new([&self.secret[..], self.inviter.as_bytes()].concat());
        write!(f, "{}#{}", self.relay, base64::encode(&bytes))
    }
}

impl fmt::Debug for Invite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invite({}, {})", self.relay, self.room_id())
    }
}

/// One peer's side of a room: everything it needs to reconnect, at any time,
/// to any relay that speaks the protocol. Persist it with [`Room::to_bytes`]
/// (or its string form), again whenever [`crate::Event::Paired`] says so,
/// and treat it as a secret.
#[derive(Clone)]
pub struct Room {
    relay: String,
    role: Role,
    keys: Keys,
    identity: StaticSecret,
    /// The other peer's identity: from the invite (joiner), or once paired (inviter).
    peer: Option<PublicKey>,
    /// Signs `JOIN` at the pair ID; known whenever `peer` is.
    pair: Option<SigningKey>,
    paired: bool,
    closing: bool,
}

/// Bumped whenever the layout changes, so an old record is refused rather
/// than misread.
const RECORD_VERSION: u8 = 2;
const HAS_PEER: u8 = 1;
const PAIRED: u8 = 2;
const CLOSING: u8 = 4;

impl Room {
    /// Creates a new room on `relay` (a `ws://` or `wss://` URL). The caller
    /// becomes the inviter; hand [`Room::invite`] to the other peer.
    pub fn create(relay: &str) -> Result<Room, Error> {
        Ok(Room::new(relay_url(relay)?, Role::Inviter, crate::random(), StaticSecret::from(crate::random()), None))
    }

    /// Accepts an invite. The caller becomes the joiner, with a new identity
    /// that the inviter will pin: persist the room before connecting, since
    /// joining again from the same invite yields a different identity, which
    /// the inviter refuses once paired.
    pub fn join(invite: &Invite) -> Room {
        let identity = StaticSecret::from(crate::random());
        Room::new(invite.relay.clone(), Role::Joiner, *invite.secret, identity, Some(invite.inviter))
    }

    fn new(relay: String, role: Role, secret: [u8; 32], identity: StaticSecret, peer: Option<PublicKey>) -> Room {
        let mut room = Room {
            relay,
            role,
            keys: Keys::derive(secret),
            identity,
            peer: None,
            pair: None,
            paired: false,
            closing: false,
        };
        if let Some(peer) = peer {
            room.set_peer(peer);
        }
        room
    }

    /// The invite for this room, while the inviter has not yet paired.
    pub fn invite(&self) -> Option<Invite> {
        (self.role == Role::Inviter && !self.paired).then(|| Invite {
            relay: self.relay.clone(),
            secret: self.keys.secret.clone(),
            inviter: PublicKey::from(&self.identity),
        })
    }

    /// The room's ID on the relay: the pair ID once paired, the invite ID before.
    pub fn id(&self) -> RoomId {
        self.room_id(if self.paired { Rendezvous::Pair } else { Rendezvous::Invite })
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn relay(&self) -> &str {
        &self.relay
    }

    /// Whether pairing is complete: both peers know each other and meet at
    /// the pair ID, where the invite no longer reaches.
    pub fn is_paired(&self) -> bool {
        self.paired
    }

    /// Whether the room is being ended (see [`crate::Peer::close`]).
    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// This peer's static public key.
    pub fn public_key(&self) -> [u8; 32] {
        PublicKey::from(&self.identity).to_bytes()
    }

    /// The other peer's static public key: known to a joiner from the
    /// start, and to an inviter once paired.
    pub fn peer_key(&self) -> Option<[u8; 32]> {
        self.peer.map(|k| k.to_bytes())
    }

    pub(crate) fn identity(&self) -> &StaticSecret {
        &self.identity
    }

    pub(crate) fn peer(&self) -> Option<&PublicKey> {
        self.peer.as_ref()
    }

    pub(crate) fn psk(&self) -> &[u8; 32] {
        &self.keys.psk
    }

    /// Records that pairing is complete; an inviter learns the joiner's key here.
    pub(crate) fn set_paired(&mut self, peer: PublicKey) {
        self.set_peer(peer);
        self.paired = true;
    }

    pub(crate) fn set_closing(&mut self) {
        self.closing = true;
    }

    fn set_peer(&mut self, peer: PublicKey) {
        // Both peers compute the same secret from their identities; anyone
        // else, invite or not, cannot.
        let shared = Zeroizing::new(self.identity.diffie_hellman(&peer).to_bytes());
        let seed = Zeroizing::new(hmac(&self.keys.secret[..], &[b"peerconnect/v1 pair", &shared[..]]));
        self.pair = Some(SigningKey::from_bytes(&seed));
        self.peer = Some(peer);
    }

    fn signer(&self, at: Rendezvous) -> &SigningKey {
        match at {
            Rendezvous::Invite => &self.keys.invite,
            Rendezvous::Pair => self.pair.as_ref().expect("the pair ID needs the other peer's key"),
        }
    }

    pub(crate) fn room_id(&self, at: Rendezvous) -> RoomId {
        RoomId(self.signer(at).verifying_key().to_bytes())
    }

    /// The WebSocket URL of [`Room::id`] on the room's relay.
    pub fn endpoint(&self) -> String {
        format!("{}/v1/{}", self.relay, self.id())
    }

    /// The WebSocket URL of the room at `at` on its relay.
    pub(crate) fn endpoint_at(&self, at: Rendezvous) -> String {
        format!("{}/v1/{}", self.relay, self.room_id(at))
    }

    /// The `JOIN` frame answering a relay's `HELLO` nonce at `at`.
    pub(crate) fn join_frame(&self, at: Rendezvous, nonce: &[u8; wire::NONCE_LEN]) -> [u8; wire::JOIN_LEN] {
        let signature = self.signer(at).sign(&wire::join_message(nonce, self.role));
        let mut frame = [0u8; wire::JOIN_LEN];
        frame[0] = wire::JOIN;
        frame[1] = self.role as u8;
        frame[2..].copy_from_slice(&signature.to_bytes());
        frame
    }

    /// Serializes the record:
    /// `version role flags secret[32] identity[32] [peer[32]] relay`.
    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let peer = self.peer;
        let flags = peer.map_or(0, |_| HAS_PEER)
            | if self.paired { PAIRED } else { 0 }
            | if self.closing { CLOSING } else { 0 };
        let mut out = Zeroizing::new(Vec::with_capacity(100 + self.relay.len()));
        out.extend_from_slice(&[RECORD_VERSION, self.role as u8, flags]);
        out.extend_from_slice(&self.keys.secret[..]);
        out.extend_from_slice(self.identity.as_bytes());
        if let Some(peer) = peer {
            out.extend_from_slice(peer.as_bytes());
        }
        out.extend_from_slice(self.relay.as_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Room, Error> {
        let bad = || Error::InvalidRecord;
        let take = |at: usize| -> Result<[u8; 32], Error> {
            bytes.get(at..at + 32).ok_or_else(bad).map(|b| b.try_into().unwrap())
        };
        if bytes.len() < 67 || bytes[0] != RECORD_VERSION || bytes[2] & !(HAS_PEER | PAIRED | CLOSING) != 0 {
            return Err(bad());
        }
        let (role, flags) = (Role::from_byte(bytes[1]).ok_or_else(bad)?, bytes[2]);
        let (peer, rest) = if flags & HAS_PEER != 0 { (Some(PublicKey::from(take(67)?)), 99) } else { (None, 67) };
        // A joiner always knows the inviter; an inviter knows the joiner exactly when paired.
        let consistent = match role {
            Role::Joiner => peer.is_some(),
            Role::Inviter => peer.is_some() == (flags & PAIRED != 0),
        };
        if !consistent || peer.is_some_and(|p| !usable(&p)) {
            return Err(bad());
        }
        let relay = std::str::from_utf8(bytes.get(rest..).ok_or_else(bad)?).map_err(|_| bad())?;
        let mut room = Room::new(relay_url(relay)?, role, take(3)?, StaticSecret::from(take(35)?), peer);
        room.paired = flags & PAIRED != 0;
        room.closing = flags & CLOSING != 0;
        Ok(room)
    }
}

/// The string form is base64url of [`Room::to_bytes`].
impl fmt::Display for Room {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&base64::encode(&self.to_bytes()))
    }
}

impl FromStr for Room {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let bytes = Zeroizing::new(base64::decode(s.trim()).ok_or(Error::InvalidRecord)?);
        Room::from_bytes(&bytes)
    }
}

impl fmt::Debug for Room {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Room")
            .field("id", &self.id())
            .field("role", &self.role)
            .field("relay", &self.relay)
            .field("paired", &self.paired)
            .field("closing", &self.closing)
            .finish_non_exhaustive()
    }
}

/// Whether an X25519 public key can take part in a handshake: low-order
/// points, which would make a Diffie-Hellman output all zeros, cannot.
fn usable(key: &PublicKey) -> bool {
    // Clamped scalars are multiples of the cofactor, so any low-order point yields zero.
    StaticSecret::from([1; 32]).diffie_hellman(key).was_contributory()
}

/// Validates a relay base URL and strips any trailing slash.
fn relay_url(url: &str) -> Result<String, Error> {
    let rest = url.strip_prefix("wss://").or_else(|| url.strip_prefix("ws://")).ok_or(Error::InvalidRelayUrl)?;
    let host = rest.split('/').next().unwrap_or_default();
    let valid = !host.is_empty() && rest.bytes().all(|b| b.is_ascii_graphic() && !matches!(b, b'?' | b'#'));
    if !valid {
        return Err(Error::InvalidRelayUrl);
    }
    Ok(url.trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_round_trip_and_room_derivation() {
        let inviter = Room::create("wss://relay.example.com/").unwrap();
        assert_eq!(inviter.relay(), "wss://relay.example.com");
        let text = inviter.invite().unwrap().to_string();
        assert!(text.starts_with("wss://relay.example.com#"));
        assert_eq!(text.len(), "wss://relay.example.com#".len() + 86);

        let invite: Invite = text.parse().unwrap();
        let joiner = Room::join(&invite);
        assert_eq!(joiner.id(), inviter.id());
        assert_eq!(invite.room_id(), inviter.id());
        assert_eq!(joiner.psk(), inviter.psk());
        assert_eq!(joiner.peer_key(), Some(inviter.public_key()));
        assert_eq!(joiner.endpoint_at(Rendezvous::Invite), format!("wss://relay.example.com/v1/{}", inviter.id()));
        assert!(joiner.invite().is_none());
    }

    #[test]
    fn pair_id_is_shared_by_the_two_peers_only() {
        let mut inviter = Room::create("wss://r.example").unwrap();
        let invite = inviter.invite().unwrap();
        let mut joiner = Room::join(&invite);
        inviter.set_paired(PublicKey::from(joiner.public_key()));
        joiner.set_paired(PublicKey::from(inviter.public_key()));
        assert_eq!(inviter.id(), joiner.id());
        assert_eq!(inviter.id(), inviter.room_id(Rendezvous::Pair));
        assert_ne!(inviter.id(), invite.room_id());
        assert!(inviter.invite().is_none());

        // Someone else with the invite, even knowing both public keys, gets
        // a different pair ID: it depends on a Diffie-Hellman secret.
        let mut stranger = Room::join(&invite);
        stranger.set_peer(PublicKey::from(inviter.public_key()));
        assert_ne!(stranger.room_id(Rendezvous::Pair), inviter.id());
    }

    #[test]
    fn record_round_trip() {
        let mut inviter = Room::create("ws://127.0.0.1:8080/relay").unwrap();
        let mut joiner = Room::join(&inviter.invite().unwrap());
        for step in 0..3 {
            for room in [&inviter, &joiner] {
                let back: Room = room.to_string().parse().unwrap();
                assert_eq!(back.to_bytes(), room.to_bytes());
                assert_eq!(
                    (back.id(), back.is_paired(), back.is_closing()),
                    (room.id(), room.is_paired(), room.is_closing())
                );
            }
            match step {
                0 => {
                    inviter.set_paired(PublicKey::from(joiner.public_key()));
                    joiner.set_paired(PublicKey::from(inviter.public_key()));
                }
                _ => {
                    inviter.set_closing();
                    joiner.set_closing();
                }
            }
        }
        assert!(inviter.invite().is_none());
        // An inviter record claiming a peer without being paired is inconsistent.
        let mut raw = Room::create("wss://r.example").unwrap().to_bytes().to_vec();
        raw[2] = HAS_PEER;
        raw.splice(67..67, [9u8; 32]);
        assert_eq!(Room::from_bytes(&raw).err(), Some(Error::InvalidRecord));
    }

    #[test]
    fn join_signature_verifies_only_for_its_nonce_and_role() {
        let room = Room::create("wss://r.example").unwrap();
        let id: RoomId = room.id().to_string().parse().unwrap();
        let frame = room.join_frame(Rendezvous::Invite, &[1; 32]);
        assert_eq!(id.verify_join(&[1; 32], &frame), Ok(Role::Inviter));
        assert_eq!(id.verify_join(&[2; 32], &frame), Err(close::UNAUTHORIZED));
        let mut other_role = frame;
        other_role[1] = 1;
        assert_eq!(id.verify_join(&[1; 32], &other_role), Err(close::UNAUTHORIZED));
        other_role[1] = 2;
        assert_eq!(id.verify_join(&[1; 32], &other_role), Err(close::PROTOCOL_ERROR));
        let stranger = Room::create("wss://r.example").unwrap().id();
        assert_eq!(stranger.verify_join(&[1; 32], &frame), Err(close::UNAUTHORIZED));
        assert_eq!(id.verify_join(&[1; 32], &frame[..65]), Err(close::PROTOCOL_ERROR));
    }

    #[test]
    fn rejects_bad_inputs() {
        for url in ["", "http://x", "wss://", "wss:///path", "wss://h?q=1", "wss://h#f", "wss://h h"] {
            assert!(Room::create(url).is_err(), "{url:?}");
        }
        let good = Room::create("wss://h").unwrap().invite().unwrap().to_string();
        for bad in [&good[..good.len() - 1], "wss://h", "wss://h#", &good.replace("wss", "https")] {
            assert!(bad.parse::<Invite>().is_err(), "{bad:?}");
        }
        // An inviter key of low order (here u = 0 and u = 1) could never complete a handshake.
        for low_order in [[0u8; 32], {
            let mut k = [0u8; 32];
            k[0] = 1;
            k
        }] {
            let fragment = base64::encode(&[[7u8; 32], low_order].concat());
            assert_eq!(format!("wss://h#{fragment}").parse::<Invite>().err(), Some(Error::InvalidInvite));
        }
        assert!("AAAA".parse::<RoomId>().is_err());
        assert!("".parse::<Room>().is_err());
    }
}
