//! Conformance test vectors (`test-vectors/v1.json`).
//!
//! Every value is computed from fixed inputs by the same code paths the
//! protocol uses. The test fails if the checked-in file differs; run it with
//! `PEERCONNECT_WRITE_VECTORS=1` to regenerate the file instead.

use std::fmt::Write;

use x25519_dalek::{PublicKey, StaticSecret};

use crate::noise::{self, Initiator};
use crate::room::Rendezvous;
use crate::wire::{self, Role};
use crate::{Invite, Room, base64};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/v1.json");
const RELAY: &str = "wss://relay.example.com";

fn bytes(seed: u8) -> [u8; 32] {
    std::array::from_fn(|i| seed.wrapping_add(i as u8))
}

fn hex(data: &[u8]) -> String {
    data.iter().fold(String::new(), |mut s, b| {
        write!(s, "{b:02x}").unwrap();
        s
    })
}

/// A room record built from fixed parts (see `Room::to_bytes`).
fn record(role: Role, paired: bool, secret: [u8; 32], identity: [u8; 32], peer: Option<[u8; 32]>) -> Room {
    let flags = peer.map_or(0, |_| 1) | if paired { 2 } else { 0 };
    let mut raw = vec![2, role as u8, flags];
    raw.extend_from_slice(&secret);
    raw.extend_from_slice(&identity);
    if let Some(peer) = peer {
        raw.extend_from_slice(&peer);
    }
    raw.extend_from_slice(RELAY.as_bytes());
    Room::from_bytes(&raw).unwrap()
}

/// The handshake payload: `stream base seen_stream seen_next`, big-endian u64s.
fn resume(fields: [u64; 4]) -> Vec<u8> {
    fields.iter().flat_map(|f| f.to_be_bytes()).collect()
}

/// Encrypts one message the way `Peer` does: a FINAL chunk, preceded by
/// MORE chunks if it is long.
fn seal_message(transport: &mut noise::Transport, data: &[u8]) -> Vec<Vec<u8>> {
    let chunks: Vec<&[u8]> =
        if data.is_empty() { vec![&[]] } else { data.chunks(noise::MAX_PLAINTEXT_LEN - 1).collect() };
    let last = chunks.len() - 1;
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| seal(transport, if i == last { 0x00 } else { 0x01 }, chunk))
        .collect()
}

fn seal(transport: &mut noise::Transport, kind: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![wire::MESSAGE];
    transport.encrypt(&[&[kind], data].concat(), &mut out).unwrap();
    out
}

fn generate() -> String {
    let secret = bytes(0x00);
    let inviter_identity = bytes(0x20);
    let joiner_identity = bytes(0x40);
    let joiner_ephemeral = bytes(0x60);
    let inviter_ephemeral = bytes(0x80);
    let nonce = bytes(0xa0);

    let inviter_public = PublicKey::from(&StaticSecret::from(inviter_identity)).to_bytes();
    let joiner_public = PublicKey::from(&StaticSecret::from(joiner_identity)).to_bytes();
    let unpaired = record(Role::Inviter, false, secret, inviter_identity, None);
    let invite = unpaired.invite().unwrap().to_string();
    let joined = Room::join(&invite.parse::<Invite>().unwrap());
    assert_eq!(joined.peer_key(), Some(inviter_public));
    // The same room, paired, on both sides.
    let inviter = record(Role::Inviter, true, secret, inviter_identity, Some(joiner_public));
    let joiner = record(Role::Joiner, true, secret, joiner_identity, Some(inviter_public));
    let shared = StaticSecret::from(joiner_identity).diffie_hellman(&PublicKey::from(inviter_public)).to_bytes();
    assert_eq!(inviter.room_id(Rendezvous::Pair), joiner.room_id(Rendezvous::Pair));
    assert_eq!(inviter.room_id(Rendezvous::Invite), unpaired.id());

    // A reconnection in mid-stream: the joiner (stream J) still holds its
    // messages from 3 on and has received the inviter's below 2; the inviter
    // (stream I) holds its messages from 2 on and has received the joiner's
    // below 5. So the joiner resends from 5, and the inviter from 2.
    let (j, i) = (0x0102_0304_0506_0708u64, 0x1112_1314_1516_1718u64);
    let joiner_resume = resume([j, 3, i, 2]);
    let inviter_resume = resume([i, 2, j, 5]);

    let (initiator, init) = Initiator::start(
        joiner.identity(),
        &PublicKey::from(inviter_public),
        joiner.psk(),
        StaticSecret::from(joiner_ephemeral),
        &joiner_resume,
    )
    .unwrap();
    let response = noise::respond(
        inviter.identity(),
        inviter.psk(),
        &init,
        StaticSecret::from(inviter_ephemeral),
        &inviter_resume,
    )
    .unwrap();
    assert_eq!(response.peer.to_bytes(), joiner_public);
    assert_eq!(response.payload, joiner_resume);
    let (mut to_inviter, reply_payload) = initiator.finish(&response.reply).unwrap();
    assert_eq!(reply_payload, inviter_resume);
    let mut to_joiner = response.transport;

    let long: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let mut messages = String::new();
    let mut message = |direction: &str, what: &str, frames: Vec<Vec<u8>>, hashed: bool| {
        use sha2::{Digest, Sha256};
        let frames: Vec<String> =
            frames.iter().map(|f| format!("\"{}\"", if hashed { hex(&Sha256::digest(f)) } else { hex(f) })).collect();
        let key = if hashed { "frames_sha256" } else { "frames" };
        writeln!(
            messages,
            "    {{ \"direction\": \"{direction}\", \"plaintext\": \"{what}\", \"{key}\": [{}] }},",
            frames.join(", ")
        )
        .unwrap();
    };
    message("joiner->inviter", "ack 2", vec![seal(&mut to_inviter, 0x03, &2u64.to_be_bytes())], false);
    message("joiner->inviter", "message 5: utf8 hello", seal_message(&mut to_inviter, b"hello"), false);
    message("inviter->joiner", "message 2: empty", seal_message(&mut to_joiner, b""), false);
    message(
        "joiner->inviter",
        "message 6: 100000 bytes, byte k = k mod 251",
        seal_message(&mut to_inviter, &long),
        true,
    );
    message("inviter->joiner", "ack 7", vec![seal(&mut to_joiner, 0x03, &7u64.to_be_bytes())], false);
    message("inviter->joiner", "message 3: close", vec![seal(&mut to_joiner, 0x02, &[])], false);
    let messages = messages.trim_end().trim_end_matches(',');

    format!(
        r#"{{
  "description": "PeerConnect v1 test vectors. Byte strings are hex unless named *_text. Key pairs are given by their private keys; X25519 private keys are used as-is (clamping happens inside X25519).",
  "relay_url": "{RELAY}",
  "room_secret": "{secret}",
  "invite_seed": "{invite_seed}",
  "invite_id": "{invite_id}",
  "invite_id_text": "{invite_id_text}",
  "psk": "{psk}",
  "invite_endpoint": "{invite_endpoint}",
  "inviter_identity_private": "{inviter_identity}",
  "inviter_identity_public": "{inviter_public}",
  "invite": "{invite}",
  "joiner_identity_private": "{joiner_identity}",
  "joiner_identity_public": "{joiner_public}",
  "identity_dh": "{shared}",
  "pair_seed": "{pair_seed}",
  "pair_id": "{pair_id}",
  "pair_id_text": "{pair_id_text}",
  "hello_nonce": "{nonce}",
  "hello_frame": "{hello}",
  "join_frame_invite_inviter": "{join_invite_inviter}",
  "join_frame_invite_joiner": "{join_invite_joiner}",
  "join_frame_pair_inviter": "{join_pair_inviter}",
  "join_frame_pair_joiner": "{join_pair_joiner}",
  "joiner_ephemeral_private": "{joiner_ephemeral}",
  "inviter_ephemeral_private": "{inviter_ephemeral}",
  "init_payload": "{joiner_resume}",
  "reply_payload": "{inviter_resume}",
  "init_frame": "{init_frame}",
  "reply_frame": "{reply_frame}",
  "messages": [
{messages}
  ]
}}
"#,
        secret = hex(&secret),
        invite_seed = hex(&noise::hmac(&secret, &[b"peerconnect/v1 room"])),
        invite_id = hex(inviter.room_id(Rendezvous::Invite).as_bytes()),
        invite_id_text = inviter.room_id(Rendezvous::Invite),
        psk = hex(inviter.psk()),
        invite_endpoint = inviter.endpoint_at(Rendezvous::Invite),
        inviter_identity = hex(&inviter_identity),
        inviter_public = hex(&inviter_public),
        joiner_identity = hex(&joiner_identity),
        joiner_public = hex(&joiner_public),
        shared = hex(&shared),
        pair_seed = hex(&noise::hmac(&secret, &[b"peerconnect/v1 pair", &shared])),
        pair_id = hex(inviter.room_id(Rendezvous::Pair).as_bytes()),
        pair_id_text = inviter.room_id(Rendezvous::Pair),
        nonce = hex(&nonce),
        hello = hex(&wire::hello(&nonce)),
        join_invite_inviter = hex(&inviter.join_frame(Rendezvous::Invite, &nonce)),
        join_invite_joiner = hex(&joiner.join_frame(Rendezvous::Invite, &nonce)),
        join_pair_inviter = hex(&inviter.join_frame(Rendezvous::Pair, &nonce)),
        join_pair_joiner = hex(&joiner.join_frame(Rendezvous::Pair, &nonce)),
        joiner_ephemeral = hex(&joiner_ephemeral),
        inviter_ephemeral = hex(&inviter_ephemeral),
        joiner_resume = hex(&joiner_resume),
        inviter_resume = hex(&inviter_resume),
        init_frame = hex(&[&[wire::INIT], &init[..]].concat()),
        reply_frame = hex(&[&[wire::REPLY], &response.reply[..]].concat()),
    )
}

#[test]
fn test_vectors_are_current() {
    let vectors = generate();
    if std::env::var_os("PEERCONNECT_WRITE_VECTORS").is_some() {
        std::fs::write(PATH, &vectors).unwrap();
    }
    let on_disk = std::fs::read_to_string(PATH).expect("test-vectors/v1.json (set PEERCONNECT_WRITE_VECTORS=1)");
    assert!(on_disk == vectors, "test-vectors/v1.json is out of date");
}

#[test]
fn invite_in_vectors_round_trips() {
    let vectors = generate();
    let invite = vectors.lines().find(|l| l.contains("\"invite\"")).unwrap();
    let invite = invite.split('"').nth(3).unwrap();
    let fragment = invite.split_once('#').unwrap().1;
    assert_eq!(base64::decode(fragment).unwrap().len(), 64);
    assert!(invite.parse::<Invite>().is_ok());
}
