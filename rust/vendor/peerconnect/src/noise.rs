//! `Noise_IKpsk1_25519_ChaChaPoly_SHA256`, the only handshake PeerConnect uses.
//!
//! ```text
//! IKpsk1:
//!   <- s
//!   ...
//!   -> e, es, s, ss, psk
//!   <- e, ee, se
//! ```
//!
//! The joiner is the initiator and the inviter the responder; the responder's
//! static key is known to the initiator in advance (from the invite). This is
//! a straight-line implementation of exactly that pattern, following revision
//! 34 of the Noise specification. Tests check it byte-for-byte against `snow`.

use chacha20poly1305::{AeadInOut, ChaCha20Poly1305, KeyInit};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

const PROTOCOL_NAME: &[u8] = b"Noise_IKpsk1_25519_ChaChaPoly_SHA256";
pub const PROLOGUE: &[u8] = b"peerconnect/v1";

pub const KEY_LEN: usize = 32;
pub const TAG_LEN: usize = 16;
/// Largest Noise message, handshake or transport.
pub const MAX_MESSAGE_LEN: usize = 65535;
/// Largest plaintext a single transport message can carry.
pub const MAX_PLAINTEXT_LEN: usize = MAX_MESSAGE_LEN - TAG_LEN;

/// Length of handshake message 1 and 2 with an empty payload.
pub const INIT_LEN: usize = KEY_LEN + (KEY_LEN + TAG_LEN) + TAG_LEN;
pub const REPLY_LEN: usize = KEY_LEN + TAG_LEN;

/// A handshake or transport message failed to authenticate or was malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error;

/// Initiator side, waiting for the responder's reply.
#[derive(Clone)]
pub struct Initiator {
    state: SymmetricState,
    e: StaticSecret,
    s: StaticSecret,
}

impl Initiator {
    /// Starts a handshake towards `rs`, returning the state and the first
    /// message, which carries `payload`.
    pub fn start(
        s: &StaticSecret,
        rs: &PublicKey,
        psk: &[u8; 32],
        e: StaticSecret,
        payload: &[u8],
    ) -> Result<(Self, Vec<u8>), Error> {
        let mut st = SymmetricState::new();
        st.mix_hash(rs.as_bytes());

        let mut msg = Vec::with_capacity(INIT_LEN);
        let e_pub = PublicKey::from(&e);
        msg.extend_from_slice(e_pub.as_bytes());
        st.mix_hash(e_pub.as_bytes());
        st.mix_key(e_pub.as_bytes());
        st.mix_key(&dh(&e, rs)?);
        st.encrypt_and_hash(PublicKey::from(s).as_bytes(), &mut msg)?;
        st.mix_key(&dh(s, rs)?);
        st.mix_key_and_hash(psk);
        st.encrypt_and_hash(payload, &mut msg)?;

        Ok((Initiator { state: st, e, s: s.clone() }, msg))
    }

    /// Processes the responder's reply, returning the session and the
    /// reply's payload. On failure the state is left untouched, so a stray
    /// or forged reply does not spoil a handshake in progress.
    pub fn finish(&self, msg: &[u8]) -> Result<(Transport, Vec<u8>), Error> {
        if msg.len() < REPLY_LEN {
            return Err(Error);
        }
        let mut st = self.state.clone();
        let re = PublicKey::from(<[u8; 32]>::try_from(&msg[..KEY_LEN]).unwrap());
        st.mix_hash(re.as_bytes());
        st.mix_key(re.as_bytes());
        st.mix_key(&dh(&self.e, &re)?);
        st.mix_key(&dh(&self.s, &re)?);
        let payload = st.decrypt_and_hash(&msg[KEY_LEN..])?;
        let (send, recv) = st.split();
        Ok((Transport { send, recv }, payload))
    }
}

/// Responder side, having read the initiator's first message.
pub struct Responder {
    state: SymmetricState,
    re: PublicKey,
    /// The initiator's static public key, authenticated by the handshake.
    pub peer: PublicKey,
    /// The payload of the initiator's first message.
    pub payload: Vec<u8>,
}

impl Responder {
    /// Reads the initiator's first message.
    pub fn read(s: &StaticSecret, psk: &[u8; 32], msg: &[u8]) -> Result<Responder, Error> {
        if msg.len() < INIT_LEN {
            return Err(Error);
        }
        let mut st = SymmetricState::new();
        st.mix_hash(PublicKey::from(s).as_bytes());

        let re = PublicKey::from(<[u8; 32]>::try_from(&msg[..KEY_LEN]).unwrap());
        st.mix_hash(re.as_bytes());
        st.mix_key(re.as_bytes());
        st.mix_key(&dh(s, &re)?);
        let rs = st.decrypt_and_hash(&msg[KEY_LEN..KEY_LEN * 2 + TAG_LEN])?;
        let rs = PublicKey::from(<[u8; 32]>::try_from(rs.as_slice()).unwrap());
        st.mix_key(&dh(s, &rs)?);
        st.mix_key_and_hash(psk);
        let payload = st.decrypt_and_hash(&msg[KEY_LEN * 2 + TAG_LEN..])?;
        Ok(Responder { state: st, re, peer: rs, payload })
    }

    /// Writes the reply, which carries `payload`, and completes the handshake.
    pub fn reply(self, e: StaticSecret, payload: &[u8]) -> Result<(Vec<u8>, Transport), Error> {
        let mut st = self.state;
        let mut reply = Vec::with_capacity(REPLY_LEN + payload.len());
        let e_pub = PublicKey::from(&e);
        reply.extend_from_slice(e_pub.as_bytes());
        st.mix_hash(e_pub.as_bytes());
        st.mix_key(e_pub.as_bytes());
        st.mix_key(&dh(&e, &self.re)?);
        st.mix_key(&dh(&e, &self.peer)?);
        st.encrypt_and_hash(payload, &mut reply)?;
        let (recv, send) = st.split();
        Ok((reply, Transport { send, recv }))
    }
}

/// The outcome of a responder processing an initiator's first message.
#[cfg(test)]
pub struct Response {
    /// The initiator's static public key, authenticated by the handshake.
    pub peer: PublicKey,
    /// The payload of the initiator's first message.
    pub payload: Vec<u8>,
    /// The reply to send back.
    pub reply: Vec<u8>,
    pub transport: Transport,
}

/// Responder side in one step: reads the first message and produces the
/// reply, which carries `payload`.
#[cfg(test)]
pub fn respond(
    s: &StaticSecret,
    psk: &[u8; 32],
    msg: &[u8],
    e: StaticSecret,
    payload: &[u8],
) -> Result<Response, Error> {
    let responder = Responder::read(s, psk, msg)?;
    let (peer, initiator_payload) = (responder.peer, responder.payload.clone());
    let (reply, transport) = responder.reply(e, payload)?;
    Ok(Response { peer, payload: initiator_payload, reply, transport })
}

/// The pair of cipher states produced by a completed handshake.
pub struct Transport {
    send: CipherState,
    recv: CipherState,
}

impl Transport {
    /// Appends the encryption of `plaintext` (at most [`MAX_PLAINTEXT_LEN`] bytes) to `out`.
    pub fn encrypt(&mut self, plaintext: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
        if plaintext.len() > MAX_PLAINTEXT_LEN {
            return Err(Error);
        }
        self.send.encrypt(&[], plaintext, out)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        if ciphertext.len() > MAX_MESSAGE_LEN {
            return Err(Error);
        }
        self.recv.decrypt(&[], ciphertext)
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct CipherState {
    k: [u8; 32],
    n: u64,
}

impl CipherState {
    fn nonce(&self) -> Result<chacha20poly1305::Nonce, Error> {
        // 2^64-1 is reserved by Noise; a session never gets anywhere close.
        if self.n == u64::MAX {
            return Err(Error);
        }
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.n.to_le_bytes());
        Ok(nonce.into())
    }

    fn encrypt(&mut self, ad: &[u8], plaintext: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
        let nonce = self.nonce()?;
        let start = out.len();
        out.extend_from_slice(plaintext);
        let tag = ChaCha20Poly1305::new(&self.k.into())
            .encrypt_inout_detached(&nonce, ad, (&mut out[start..]).into())
            .map_err(|_| Error)?;
        out.extend_from_slice(&tag);
        self.n += 1;
        Ok(())
    }

    fn decrypt(&mut self, ad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = self.nonce()?;
        let len = ciphertext.len().checked_sub(TAG_LEN).ok_or(Error)?;
        let mut plaintext = ciphertext[..len].to_vec();
        let tag = <[u8; TAG_LEN]>::try_from(&ciphertext[len..]).unwrap();
        ChaCha20Poly1305::new(&self.k.into())
            .decrypt_inout_detached(&nonce, ad, plaintext.as_mut_slice().into(), &tag.into())
            .map_err(|_| Error)?;
        self.n += 1;
        Ok(plaintext)
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct SymmetricState {
    ck: [u8; 32],
    h: [u8; 32],
    cipher: Option<CipherState>,
}

impl SymmetricState {
    fn new() -> Self {
        // The protocol name is longer than HASHLEN, so h = HASH(name).
        let h: [u8; 32] = Sha256::digest(PROTOCOL_NAME).into();
        let mut st = SymmetricState { ck: h, h, cipher: None };
        st.mix_hash(PROLOGUE);
        st
    }

    fn mix_hash(&mut self, data: &[u8]) {
        self.h = Sha256::new().chain_update(self.h).chain_update(data).finalize().into();
    }

    fn mix_key(&mut self, ikm: &[u8]) {
        let [ck, k] = hkdf(&self.ck, ikm);
        self.ck = ck;
        self.cipher = Some(CipherState { k, n: 0 });
    }

    fn mix_key_and_hash(&mut self, ikm: &[u8]) {
        let [ck, h, k] = hkdf(&self.ck, ikm);
        self.ck = ck;
        self.mix_hash(&h);
        self.cipher = Some(CipherState { k, n: 0 });
    }

    fn encrypt_and_hash(&mut self, plaintext: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
        let start = out.len();
        // In IKpsk1 a key is always established before the first encryption.
        let h = self.h;
        self.cipher.as_mut().ok_or(Error)?.encrypt(&h, plaintext, out)?;
        self.mix_hash(&out[start..]);
        Ok(())
    }

    fn decrypt_and_hash(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        let h = self.h;
        let plaintext = self.cipher.as_mut().ok_or(Error)?.decrypt(&h, ciphertext)?;
        self.mix_hash(ciphertext);
        Ok(plaintext)
    }

    /// Returns (initiator→responder, responder→initiator) cipher states.
    fn split(&self) -> (CipherState, CipherState) {
        let [k1, k2] = hkdf(&self.ck, &[]);
        (CipherState { k: k1, n: 0 }, CipherState { k: k2, n: 0 })
    }
}

fn dh(secret: &StaticSecret, public: &PublicKey) -> Result<[u8; 32], Error> {
    let shared = secret.diffie_hellman(public);
    // Reject low-order points: a contributory exchange is required.
    if !shared.was_contributory() {
        return Err(Error);
    }
    Ok(shared.to_bytes())
}

pub(crate) fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

/// The Noise HKDF: HMAC-SHA256 extract-then-expand producing `N` outputs.
fn hkdf<const N: usize>(ck: &[u8; 32], ikm: &[u8]) -> [[u8; 32]; N] {
    let mut prk = hmac(ck, &[ikm]);
    let mut out = [[0u8; 32]; N];
    for i in 0..N {
        let prev: &[u8] = if i == 0 { &[] } else { &out[i - 1] };
        out[i] = hmac(&prk, &[prev, &[i as u8 + 1]]);
    }
    prk.zeroize();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> StaticSecret {
        StaticSecret::from([n; 32])
    }

    fn snow_params() -> snow::params::NoiseParams {
        "Noise_IKpsk1_25519_ChaChaPoly_SHA256".parse().unwrap()
    }

    #[test]
    fn byte_identical_to_snow() {
        let (si, sr, ei, er, psk) = (key(1), key(2), key(3), key(4), [5u8; 32]);
        let sr_pub = PublicKey::from(&sr);

        let mut snow_i = snow::Builder::new(snow_params())
            .local_private_key(&si.to_bytes())
            .unwrap()
            .remote_public_key(sr_pub.as_bytes())
            .unwrap()
            .psk(1, &psk)
            .unwrap()
            .prologue(PROLOGUE)
            .unwrap()
            .fixed_ephemeral_key_for_testing_only(&ei.to_bytes())
            .build_initiator()
            .unwrap();
        let mut snow_r = snow::Builder::new(snow_params())
            .local_private_key(&sr.to_bytes())
            .unwrap()
            .psk(1, &psk)
            .unwrap()
            .prologue(PROLOGUE)
            .unwrap()
            .fixed_ephemeral_key_for_testing_only(&er.to_bytes())
            .build_responder()
            .unwrap();

        let (ours_i, msg1) = Initiator::start(&si, &sr_pub, &psk, ei, b"first payload").unwrap();
        let mut buf = [0u8; 4096];
        let n = snow_i.write_message(b"first payload", &mut buf).unwrap();
        assert_eq!(msg1, &buf[..n]);

        let resp = respond(&sr, &psk, &msg1, er, b"second").unwrap();
        snow_r.read_message(&msg1, &mut buf).unwrap();
        let n = snow_r.write_message(b"second", &mut buf).unwrap();
        assert_eq!(resp.reply, &buf[..n]);
        assert_eq!(resp.peer.as_bytes(), PublicKey::from(&si).as_bytes());
        assert_eq!(resp.payload, b"first payload");

        let (mut ours_i, payload) = ours_i.finish(&resp.reply).unwrap();
        assert_eq!(payload, b"second");
        snow_i.read_message(&resp.reply, &mut buf).unwrap();
        let mut ours_r = resp.transport;
        let mut snow_i = snow_i.into_transport_mode().unwrap();
        let mut snow_r = snow_r.into_transport_mode().unwrap();

        for (i, text) in [&b""[..], b"hello", &[7u8; 3000]].into_iter().enumerate() {
            let mut ct = Vec::new();
            ours_i.encrypt(text, &mut ct).unwrap();
            let n = snow_i.write_message(text, &mut buf[..]).unwrap_or_else(|_| panic!("{i}"));
            assert_eq!(ct, &buf[..n]);
            assert_eq!(ours_r.decrypt(&ct).unwrap(), text);
            let n = snow_r.read_message(&ct, &mut [0u8; 4096]).unwrap();
            assert_eq!(n, text.len());

            let mut ct = Vec::new();
            ours_r.encrypt(text, &mut ct).unwrap();
            assert_eq!(ours_i.decrypt(&ct).unwrap(), text);
        }
    }

    #[test]
    fn rejects_wrong_psk_wrong_key_and_tampering() {
        let (si, sr, psk) = (key(1), key(2), [5u8; 32]);
        let sr_pub = PublicKey::from(&sr);
        let (_, msg1) = Initiator::start(&si, &sr_pub, &psk, key(3), &[]).unwrap();

        assert!(respond(&sr, &[6u8; 32], &msg1, key(4), &[]).is_err(), "wrong psk");
        assert!(respond(&key(9), &psk, &msg1, key(4), &[]).is_err(), "wrong responder");
        for i in 0..msg1.len() {
            let mut bad = msg1.clone();
            bad[i] ^= 1;
            assert!(respond(&sr, &psk, &bad, key(4), &[]).is_err(), "flipped byte {i}");
        }
        assert!(respond(&sr, &psk, &msg1[..msg1.len() - 1], key(4), &[]).is_err());

        let (init, msg1) = Initiator::start(&si, &sr_pub, &psk, key(3), &[]).unwrap();
        let resp = respond(&sr, &psk, &msg1, key(4), &[]).unwrap();
        let mut bad = resp.reply.clone();
        bad[40] ^= 1;
        assert!(init.finish(&bad).is_err());
        // A failed reply leaves the handshake usable.
        assert!(init.finish(&resp.reply).is_ok());
    }

    #[test]
    fn transport_rejects_replay_and_reorder() {
        let (si, sr, psk) = (key(1), key(2), [5u8; 32]);
        let (init, msg1) = Initiator::start(&si, &PublicKey::from(&sr), &psk, key(3), &[]).unwrap();
        let resp = respond(&sr, &psk, &msg1, key(4), &[]).unwrap();
        let (mut a, _) = init.finish(&resp.reply).unwrap();
        let mut b = resp.transport;

        let (mut m1, mut m2) = (Vec::new(), Vec::new());
        a.encrypt(b"one", &mut m1).unwrap();
        a.encrypt(b"two", &mut m2).unwrap();
        assert!(b.decrypt(&m2).is_err(), "reordered");
        assert_eq!(b.decrypt(&m1).unwrap(), b"one");
        assert!(b.decrypt(&m1).is_err(), "replayed");
    }

    #[test]
    fn rejects_low_order_ephemeral() {
        let (sr, psk) = (key(2), [5u8; 32]);
        let mut msg1 = vec![0u8; INIT_LEN];
        msg1[0] = 1; // the point u=1 has order 4
        assert_eq!(respond(&sr, &psk, &msg1, key(4), &[]).err(), Some(Error));
    }
}
