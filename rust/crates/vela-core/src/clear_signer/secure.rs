//! The end-to-end session a Clear Signer channel runs when the path between the
//! wallet and the page is not the browser's own: the tunnel (spec 075,
//! `contracts/tunnel.md`) and BLE (`app-web/clearsigning/PROTOCOL.md` §3).
//!
//! P-256 ECDH → HKDF-SHA256 → AES-256-GCM, and a six-digit code both screens
//! show — the one place a stand-in on the path is caught. Byte-identical to the
//! page's `lib/transport/secure.js`; `tests/clear-signer/secure-session.json` pins
//! both sides.
//!
//! The core holds no randomness: the shell hands in the 32 secret bytes and the
//! 16-byte nonce, as it does for every other key this crate uses.
//!
//! Roles are the BLE ones. The **signer** is the page (BLE's central), and
//! speaks first; the **requester** is the wallet (the peripheral). `c2p` is
//! signer → requester, `p2c` requester → signer.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hkdf::Hkdf;
use p256::elliptic_curve::sec1::ToSec1Point;
use p256::{PublicKey, SecretKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Which channel the session runs on. The label goes into every derivation and
/// every AAD, so a BLE message can never be replayed into a tunnel session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Tunnel,
    Ble,
}

impl Label {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Label::Tunnel => "vela-tunnel/1",
            Label::Ble => "vela-ble/1",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The page — BLE's central. Sends the first hello.
    Signer,
    /// The wallet — BLE's peripheral.
    Requester,
}

impl Role {
    const fn as_str(self) -> &'static str {
        match self {
            Role::Signer => "signer",
            Role::Requester => "requester",
        }
    }
}

/// Why a handshake or a message was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecureError {
    #[error("the secret bytes are not a P-256 scalar")]
    BadSecret,
    #[error("the peer's hello is malformed")]
    BadHello,
    #[error("the peer spoke in the wrong role")]
    WrongRole,
    #[error("the peer's key does not match the pairing link")]
    ForeignPeer,
    #[error("the message could not be opened")]
    Unreadable,
    #[error("the message was replayed or out of order")]
    Replayed,
}

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64url(text: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(text.trim_end_matches('=')).ok()
}

/// `b64url(SHA-256(pk)[0..16])` — the requester key's fingerprint the pairing
/// link carries (`rk`), so the page can refuse a stand-in wallet.
#[must_use]
pub fn key_fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    b64url(&digest[..16])
}

/// One side of the handshake, before the peer's hello has arrived.
pub struct Handshake {
    secret: SecretKey,
    public: [u8; 65],
    nonce: [u8; 16],
    role: Role,
}

/// A `data:` URI the page will accept for a peer's mark: inline, raster, and
/// small enough that a handshake stays a handshake (the hello is plaintext and
/// framed). SVG is not a picture but a document — scripts and external
/// references — so it is not one of these.
fn is_inline_raster(icon: &str) -> bool {
    const MAX: usize = 6144;
    if icon.len() > MAX {
        return false;
    }
    let Some(rest) = icon
        .strip_prefix("data:image/png;base64,")
        .or_else(|| icon.strip_prefix("data:image/jpeg;base64,"))
        .or_else(|| icon.strip_prefix("data:image/webp;base64,"))
    else {
        return false;
    };
    !rest.is_empty()
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

impl Handshake {
    /// # Errors
    /// [`SecureError::BadSecret`] when `secret` is not a non-zero P-256 scalar —
    /// a ~2⁻³² event; the shell retries with fresh randomness.
    pub fn new(secret: &[u8; 32], nonce: [u8; 16], role: Role) -> Result<Self, SecureError> {
        let secret = SecretKey::from_slice(secret).map_err(|_| SecureError::BadSecret)?;
        let point = secret.public_key().to_sec1_point(false);
        let mut public = [0u8; 65];
        public.copy_from_slice(point.as_bytes());
        Ok(Self {
            secret,
            public,
            nonce,
            role,
        })
    }

    /// The 65-byte uncompressed public key.
    #[must_use]
    pub const fn public_key(&self) -> &[u8; 65] {
        &self.public
    }

    /// This side's hello, as the plaintext text frame (`{"v":1,"t":"hello",…}`).
    #[must_use]
    pub fn hello(&self, app: Option<&str>) -> String {
        self.hello_with(app, None)
    }

    /// The same, plus a mark for the peer to draw beside the name.
    ///
    /// Both are CLAIMS — this handshake proves a shared secret, never an
    /// identity — and the page says so beside them. The mark must be a small
    /// inline raster `data:` URI: a remote URL would make the page fetch from
    /// a third party on a stranger's say-so, and the hello travels in the
    /// clear, in frames, before there is a session to seal it. Anything else
    /// is dropped here rather than sent to be refused there.
    pub fn hello_with(&self, app: Option<&str>, icon: Option<&str>) -> String {
        let mut hello = json!({
            "v": 1,
            "t": "hello",
            "role": self.role.as_str(),
            "pk": b64url(&self.public),
            "nonce": b64url(&self.nonce),
        });
        if let Some(object) = hello.as_object_mut() {
            if let Some(app) = app {
                object.insert("app".to_owned(), Value::String(app.to_owned()));
            }
            if let Some(icon) = icon.filter(|icon| is_inline_raster(icon)) {
                object.insert("icon".to_owned(), Value::String(icon.to_owned()));
            }
        }
        hello.to_string()
    }

    /// Finish with the peer's hello. `expected_fingerprint` is the page's `rk`
    /// check (the signer names the requester it was paired with); the wallet
    /// passes `None`.
    ///
    /// # Errors
    /// A malformed hello, the peer in our own role, a key off the curve, or a
    /// requester whose key does not match `expected_fingerprint`.
    pub fn complete(
        self,
        peer_hello: &str,
        label: Label,
        expected_fingerprint: Option<&str>,
    ) -> Result<Session, SecureError> {
        let hello: Value = serde_json::from_str(peer_hello).map_err(|_| SecureError::BadHello)?;
        if hello.get("t").and_then(Value::as_str) != Some("hello") {
            return Err(SecureError::BadHello);
        }
        let peer_role = match hello.get("role").and_then(Value::as_str) {
            Some("signer") => Role::Signer,
            Some("requester") => Role::Requester,
            _ => return Err(SecureError::BadHello),
        };
        if peer_role == self.role {
            return Err(SecureError::WrongRole);
        }
        let peer_pk = hello
            .get("pk")
            .and_then(Value::as_str)
            .and_then(unb64url)
            .ok_or(SecureError::BadHello)?;
        let peer_nonce = hello
            .get("nonce")
            .and_then(Value::as_str)
            .and_then(unb64url)
            .filter(|n| n.len() == 16)
            .ok_or(SecureError::BadHello)?;
        if let Some(expected) = expected_fingerprint {
            if key_fingerprint(&peer_pk) != expected {
                return Err(SecureError::ForeignPeer);
            }
        }
        let peer = PublicKey::from_sec1_bytes(&peer_pk).map_err(|_| SecureError::BadHello)?;
        let shared = p256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), peer.as_affine());

        let (signer_nonce, requester_nonce) = match self.role {
            Role::Signer => (self.nonce.as_slice(), peer_nonce.as_slice()),
            Role::Requester => (peer_nonce.as_slice(), self.nonce.as_slice()),
        };
        let salt = [signer_nonce, requester_nonce].concat();
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), shared.raw_secret_bytes().as_slice());
        let mut key = [0u8; 32];
        let mut code = [0u8; 4];
        // Expand fails only past 255·32 bytes of output; 32 and 4 never do.
        hkdf.expand(format!("{} key", label.as_str()).as_bytes(), &mut key)
            .map_err(|_| SecureError::BadHello)?;
        hkdf.expand(format!("{} code", label.as_str()).as_bytes(), &mut code)
            .map_err(|_| SecureError::BadHello)?;
        let code = format!("{:06}", u32::from_be_bytes(code) % 1_000_000);

        Ok(Session {
            key,
            code,
            label,
            role: self.role,
            sent: 0,
            received: 0,
            peer_public: peer_pk,
        })
    }
}

/// What a sealed message's AAD ends with. The tunnel binds the IV counter; BLE
/// binds its frame message id (PROTOCOL.md §3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail {
    Counter,
    MsgId(u8),
}

/// An established session: one key, one code, a counter per direction.
pub struct Session {
    key: [u8; 32],
    code: String,
    label: Label,
    role: Role,
    sent: u64,
    received: u64,
    peer_public: Vec<u8>,
}

impl Session {
    /// The six digits both screens show.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The peer's 65-byte public key (the wallet shows nothing of it; tests do).
    #[must_use]
    pub fn peer_public_key(&self) -> &[u8] {
        &self.peer_public
    }

    const fn outgoing(&self) -> &'static str {
        match self.role {
            Role::Signer => "c2p",
            Role::Requester => "p2c",
        }
    }

    const fn incoming(&self) -> &'static str {
        match self.role {
            Role::Signer => "p2c",
            Role::Requester => "c2p",
        }
    }

    fn iv(direction: &str, counter: u64) -> [u8; 12] {
        let mut iv = [0u8; 12];
        iv[..4].copy_from_slice(if direction == "c2p" { b"C2P." } else { b"P2C." });
        iv[4..].copy_from_slice(&counter.to_be_bytes());
        iv
    }

    fn aad(&self, direction: &str, counter: u64, tail: Tail) -> Vec<u8> {
        let end = match tail {
            Tail::Counter => counter.to_string(),
            Tail::MsgId(id) => id.to_string(),
        };
        format!("{}|{direction}|{end}", self.label.as_str()).into_bytes()
    }

    /// `IV(12) ‖ AES-GCM(plaintext)` — the next message this side sends.
    #[must_use]
    pub fn seal(&mut self, plaintext: &[u8], tail: Tail) -> Vec<u8> {
        self.sent += 1;
        let direction = self.outgoing();
        let iv = Self::iv(direction, self.sent);
        let aad = self.aad(direction, self.sent, tail);
        let sealed = Aes256Gcm::new_from_slice(&self.key)
            .ok()
            .and_then(|cipher| {
                cipher
                    .encrypt(
                        &Nonce::from(iv),
                        Payload {
                            msg: plaintext,
                            aad: &aad,
                        },
                    )
                    .ok()
            })
            .unwrap_or_default();
        [iv.as_slice(), sealed.as_slice()].concat()
    }

    /// Open the peer's next message. Its IV must carry the peer's direction and
    /// a counter above every one already opened — a replay or a reordering is
    /// refused, never decrypted.
    ///
    /// # Errors
    /// [`SecureError::Replayed`] for a stale counter or the wrong direction;
    /// [`SecureError::Unreadable`] when the tag does not verify.
    pub fn open(&mut self, sealed: &[u8], tail: Tail) -> Result<Vec<u8>, SecureError> {
        if sealed.len() < 12 + 16 {
            return Err(SecureError::Unreadable);
        }
        let (iv, body) = sealed.split_at(12);
        let direction = self.incoming();
        let tag: &[u8; 4] = if direction == "c2p" { b"C2P." } else { b"P2C." };
        if &iv[..4] != tag {
            return Err(SecureError::Replayed);
        }
        let mut counter_bytes = [0u8; 8];
        counter_bytes.copy_from_slice(&iv[4..]);
        let counter = u64::from_be_bytes(counter_bytes);
        if counter <= self.received {
            return Err(SecureError::Replayed);
        }
        let aad = self.aad(direction, counter, tail);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(iv);
        let plain = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| SecureError::Unreadable)?
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: body,
                    aad: &aad,
                },
            )
            .map_err(|_| SecureError::Unreadable)?;
        self.received = counter;
        Ok(plain)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {

    /// A peer's mark is a claim, so the only question here is what shape of
    /// claim may travel: inline, raster, small. A remote URL would make the
    /// page fetch from a third party on a stranger's say-so; an SVG is a
    /// document, not a picture.
    #[test]
    fn a_peer_may_send_a_small_inline_raster_mark_and_nothing_else() {
        let handshake = Handshake::new(&[7u8; 32], [9u8; 16], Role::Requester).unwrap();
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==";
        let hello: serde_json::Value =
            serde_json::from_str(&handshake.hello_with(Some("Vela Wallet 0.9.4"), Some(png)))
                .unwrap();
        assert_eq!(hello["app"], "Vela Wallet 0.9.4");
        assert_eq!(hello["icon"], png);

        for refused in [
            "https://example.com/logo.png",
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            "data:text/html;base64,PGh0bWw+",
            "data:image/png;base64,",
            "data:image/png;base64,not base64!",
        ] {
            let hello: serde_json::Value =
                serde_json::from_str(&handshake.hello_with(None, Some(refused))).unwrap();
            assert!(hello.get("icon").is_none(), "{refused} must not travel");
        }

        // And a mark too big for a plaintext, framed handshake.
        let huge = format!("data:image/png;base64,{}", "A".repeat(7000));
        let hello: serde_json::Value =
            serde_json::from_str(&handshake.hello_with(None, Some(&huge))).unwrap();
        assert!(hello.get("icon").is_none(), "6 KiB is the cap");
    }

    use super::*;

    const SIGNER_SECRET: [u8; 32] = [0x11; 32];
    const REQUESTER_SECRET: [u8; 32] = [0x22; 32];
    const SIGNER_NONCE: [u8; 16] = [0xa1; 16];
    const REQUESTER_NONCE: [u8; 16] = [0xb2; 16];

    fn pair(label: Label) -> (Session, Session) {
        let signer = Handshake::new(&SIGNER_SECRET, SIGNER_NONCE, Role::Signer).unwrap();
        let requester =
            Handshake::new(&REQUESTER_SECRET, REQUESTER_NONCE, Role::Requester).unwrap();
        let signer_hello = signer.hello(None);
        let requester_hello = requester.hello(Some("vela-test/1"));
        let rk = key_fingerprint(requester.public_key());
        let s = signer.complete(&requester_hello, label, Some(&rk)).unwrap();
        let r = requester.complete(&signer_hello, label, None).unwrap();
        (s, r)
    }

    #[test]
    fn both_ends_derive_the_same_code_and_talk() {
        let (mut signer, mut requester) = pair(Label::Tunnel);
        assert_eq!(signer.code(), requester.code());
        assert_eq!(signer.code().len(), 6);
        let intent = br#"{"v":1,"t":"intent","n":1}"#;
        let sealed = requester.seal(intent, Tail::Counter);
        assert_eq!(signer.open(&sealed, Tail::Counter).unwrap(), intent);
        let answer = br#"{"v":1,"t":"result","n":2}"#;
        let sealed = signer.seal(answer, Tail::Counter);
        assert_eq!(requester.open(&sealed, Tail::Counter).unwrap(), answer);
    }

    #[test]
    fn a_replayed_or_reflected_message_is_refused() {
        let (mut signer, mut requester) = pair(Label::Tunnel);
        let sealed = requester.seal(b"one", Tail::Counter);
        signer.open(&sealed, Tail::Counter).unwrap();
        assert_eq!(
            signer.open(&sealed, Tail::Counter),
            Err(SecureError::Replayed)
        );
        // Our own message bounced back at us carries our direction.
        let mine = signer.seal(b"two", Tail::Counter);
        assert_eq!(
            signer.open(&mine, Tail::Counter),
            Err(SecureError::Replayed)
        );
    }

    #[test]
    fn a_tampered_message_or_another_label_does_not_open() {
        let (mut signer, mut requester) = pair(Label::Tunnel);
        let mut sealed = requester.seal(b"intent", Tail::Counter);
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(
            signer.open(&sealed, Tail::Counter),
            Err(SecureError::Unreadable)
        );

        let (_, mut ble_requester) = pair(Label::Ble);
        let (mut tunnel_signer, _) = pair(Label::Tunnel);
        let crossed = ble_requester.seal(b"intent", Tail::Counter);
        assert_eq!(
            tunnel_signer.open(&crossed, Tail::Counter),
            Err(SecureError::Unreadable)
        );
    }

    #[test]
    fn the_page_refuses_a_stand_in_wallet() {
        let signer = Handshake::new(&SIGNER_SECRET, SIGNER_NONCE, Role::Signer).unwrap();
        let stranger = Handshake::new(&[0x33; 32], REQUESTER_NONCE, Role::Requester).unwrap();
        let wallet = Handshake::new(&REQUESTER_SECRET, REQUESTER_NONCE, Role::Requester).unwrap();
        let rk = key_fingerprint(wallet.public_key());
        assert!(matches!(
            signer.complete(&stranger.hello(None), Label::Tunnel, Some(&rk)),
            Err(SecureError::ForeignPeer)
        ));
    }

    #[test]
    fn a_hello_in_our_own_role_or_malformed_is_refused() {
        let signer = Handshake::new(&SIGNER_SECRET, SIGNER_NONCE, Role::Signer).unwrap();
        let other = Handshake::new(&REQUESTER_SECRET, REQUESTER_NONCE, Role::Signer).unwrap();
        assert!(matches!(
            signer.complete(&other.hello(None), Label::Tunnel, None),
            Err(SecureError::WrongRole)
        ));
        let signer = Handshake::new(&SIGNER_SECRET, SIGNER_NONCE, Role::Signer).unwrap();
        assert!(matches!(
            signer.complete(
                r#"{"t":"hello","role":"requester","pk":"AAAA","nonce":"AAAA"}"#,
                Label::Tunnel,
                None
            ),
            Err(SecureError::BadHello)
        ));
    }

    #[test]
    fn a_zero_secret_is_refused() {
        assert!(matches!(
            Handshake::new(&[0u8; 32], SIGNER_NONCE, Role::Signer),
            Err(SecureError::BadSecret)
        ));
    }
}
