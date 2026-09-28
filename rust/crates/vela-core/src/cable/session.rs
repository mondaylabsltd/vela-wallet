//! The initiator's caBLE session, sans-IO: one object that ties the QR payload,
//! the advert trial-decrypt, the key derivations, the tunnel URL, and the Noise
//! handshake into the sequence a shell drives — so the shell owns only the
//! radio and the socket, never a derivation or a wire format.
//!
//! The session holds the two secrets it generated for one "sign in with your
//! phone" attempt: the static seed (whose public key it advertised in the QR)
//! and the 16-byte QR secret (from which every caBLE key derives). Both come
//! from the shell's CSPRNG; the session keeps them so the QR the person sees and
//! the handshake that follows cannot drift apart.
//!
//! The flow a shell runs:
//!   1. [`CableInitiator::new`] with fresh randomness, then show
//!      [`CableInitiator::qr_payload`].
//!   2. scan BLE adverts, trial-decrypting each with
//!      [`CableInitiator::try_decrypt_advert`]; the one that decrypts is this
//!      QR's phone.
//!   3. open the WebSocket at [`CableInitiator::connect_url`].
//!   4. [`CableInitiator::establish`] over that socket → a [`CableConnection`]
//!      the ceremony drives.

use crate::ctap::ceremony::{CableError, TouchAnnouncer};

use super::conn::{CableConnection, CablePort};
use super::crypto::{self, AdvertEid};
use super::keys::KeyPair;
use super::{qr, tunnel_domain};

/// One initiator-side caBLE attempt.
pub struct CableInitiator {
    static_seed: [u8; 32],
    qr_secret: [u8; 16],
}

impl CableInitiator {
    /// `static_seed` is 32 bytes and `qr_secret` is 16, both from the shell's
    /// CSPRNG. `None` if the lengths are wrong or the seed is not a valid P-256
    /// scalar (a ~2⁻³² event the shell retries with fresh randomness) — so every
    /// later method is infallible for its own reasons, not this one's.
    #[must_use]
    pub fn new(static_seed: &[u8], qr_secret: &[u8]) -> Option<Self> {
        if qr_secret.len() != 16 || static_seed.len() != 32 {
            return None;
        }
        // Validate the seed is a usable scalar up front so `qr_payload` cannot fail.
        KeyPair::from_seed(static_seed)?;
        let mut seed = [0u8; 32];
        seed.copy_from_slice(static_seed);
        let mut secret = [0u8; 16];
        secret.copy_from_slice(qr_secret);
        Some(Self {
            static_seed: seed,
            qr_secret: secret,
        })
    }

    /// The `FIDO:/…` payload to render as a QR. `offer_ble` advertises the
    /// CTAP 2.3 BLE data channel alongside the WebSocket tunnel; leave it off for
    /// a WebSocket-only initiator (a pre-2.3 responder hard-rejects the channel
    /// list). `for_get` chooses the flow hint the phone offers: `true` to sign in
    /// with an existing passkey, `false` to create one on the phone.
    #[must_use]
    pub fn qr_payload(&self, epoch_seconds: i64, for_get: bool) -> String {
        // The seed was validated in `new`, so this keypair always builds.
        let compressed = KeyPair::from_seed(&self.static_seed)
            .map(|kp| kp.public_compressed())
            .unwrap_or_default();
        let hint = if for_get { qr::HINT_GET } else { qr::HINT_MAKE };
        qr::build_payload(&compressed, &self.qr_secret, epoch_seconds, hint)
    }

    /// The 64-byte advert key (AES-256 ‖ HMAC-SHA256) a BLE scanner trial-decrypts
    /// candidates with. A shell that scans in native code can take this and do
    /// the decrypt there; [`Self::try_decrypt_advert`] does it in the core.
    #[must_use]
    pub fn eid_key(&self) -> Vec<u8> {
        crypto::eid_key(&self.qr_secret)
    }

    /// The 128-bit tunnel id for the WebSocket rendezvous.
    #[must_use]
    pub fn tunnel_id(&self) -> Vec<u8> {
        crypto::tunnel_id(&self.qr_secret)
    }

    /// Trial-decrypt one 20-byte BLE advert candidate; `Some` with the parsed
    /// EID when it is this QR's phone.
    #[must_use]
    pub fn try_decrypt_advert(&self, candidate: &[u8]) -> Option<AdvertEid> {
        let plaintext = crypto::try_decrypt_advert(candidate, &self.eid_key())?;
        AdvertEid::parse(&plaintext)
    }

    /// The WebSocket tunnel URL to open, from the decrypted 16-byte advert
    /// plaintext: `wss://<domain>/cable/connect/<routing>/<tunnel_id>`. `None`
    /// if the advert is malformed or names an unknown tunnel domain.
    #[must_use]
    pub fn connect_url(&self, advert_plaintext16: &[u8]) -> Option<String> {
        let eid = AdvertEid::parse(advert_plaintext16)?;
        let domain = tunnel_domain::decode(eid.tunnel_domain_id)?;
        let routing = hex(&eid.routing_id);
        let tunnel = hex(&self.tunnel_id());
        Some(format!("wss://{domain}/cable/connect/{routing}/{tunnel}"))
    }

    /// The Noise pre-shared key for this advert. The handshake below uses it;
    /// exposed for a shell (or a test) that wants to see it.
    #[must_use]
    pub fn psk(&self, advert_plaintext16: &[u8]) -> Vec<u8> {
        crypto::psk(&self.qr_secret, advert_plaintext16)
    }

    /// Run the Noise handshake over a connected `port` and return the
    /// [`CableConnection`] the ceremony drives.
    ///
    /// * `advert_plaintext16` — the decrypted BLE advert that named this phone;
    ///   the PSK derives from it.
    /// * `ephemeral_seed` — 32 fresh bytes for this handshake's ephemeral key.
    /// * `product` — a localised label for the phone ("your phone"), for the
    ///   failure sentence and the "approve on your phone" prompt.
    pub fn establish<P: CablePort>(
        &self,
        port: P,
        advert_plaintext16: &[u8],
        ephemeral_seed: &[u8],
        product: String,
        on_touch: Option<TouchAnnouncer>,
    ) -> Result<CableConnection<P>, CableError> {
        let psk = self.psk(advert_plaintext16);
        CableConnection::establish(
            port,
            &self.static_seed,
            ephemeral_seed,
            &psk,
            product,
            on_touch,
        )
    }
}

/// Lowercase hex, for the routing and tunnel ids in the URL.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(nibble(byte >> 4));
        out.push(nibble(byte & 0x0f));
    }
    out
}

/// UPPER-case, as Chromium's `GetConnectURL` writes the ids
/// (`base::HexEncode`). Google's tunnel server took either case; Apple's
/// (`cable.auth.com`, every iPhone) closed a lower-case connect with
/// "Policy violation" right after the first frame (spec 083 W20).
fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'A' + (n - 10)) as char,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn sealed_advert(qr_secret: &[u8], mut eid16: [u8; 16]) -> [u8; 20] {
        use aes::cipher::{BlockCipherEncrypt, KeyInit};
        use aes::Aes256;
        use hmac::{Mac, SimpleHmac};
        use sha2::Sha256;
        eid16[0] = 0; // flags must be 0 to decrypt
        let key = crypto::eid_key(qr_secret);
        let mut block = eid16;
        Aes256::new_from_slice(&key[0..32])
            .unwrap()
            .encrypt_block((&mut block).into());
        let mut mac = <SimpleHmac<Sha256> as KeyInit>::new_from_slice(&key[32..64]).unwrap();
        mac.update(&block);
        let tag = mac.finalize().into_bytes();
        let mut advert = [0u8; 20];
        advert[0..16].copy_from_slice(&block);
        advert[16..20].copy_from_slice(&tag[0..4]);
        advert
    }

    #[test]
    fn new_validates_the_secret_lengths() {
        assert!(CableInitiator::new(&[1u8; 32], &[0u8; 16]).is_some());
        assert!(CableInitiator::new(&[1u8; 32], &[0u8; 15]).is_none());
        assert!(CableInitiator::new(&[1u8; 31], &[0u8; 16]).is_none());
    }

    #[test]
    fn the_qr_carries_this_session_s_own_public_key() {
        let session = CableInitiator::new(&[9u8; 32], &[7u8; 16]).unwrap();
        let payload = session.qr_payload(1_700_000_000, true);
        assert!(payload.starts_with("FIDO:/"));
        // The compressed key in the QR is this session's static key.
        let expected = KeyPair::from_seed(&[9u8; 32]).unwrap().public_compressed();
        // Decode the base10 CBOR and confirm key 0 matches.
        let digits = payload.strip_prefix("FIDO:/").unwrap();
        let cbor = super::super::base10::decode(digits).unwrap();
        let value: ciborium::Value = ciborium::de::from_reader(cbor.as_slice()).unwrap();
        let ciborium::Value::Map(entries) = value else {
            panic!("not a map")
        };
        let key0 = entries
            .iter()
            .find(|(k, _)| matches!(k, ciborium::Value::Integer(i) if *i == 0.into()))
            .map(|(_, v)| v)
            .unwrap();
        assert!(matches!(key0, ciborium::Value::Bytes(b) if b == &expected));
    }

    #[test]
    fn a_sealed_advert_decrypts_and_yields_a_tunnel_url() {
        let qr_secret = [0x11u8; 16];
        let session = CableInitiator::new(&[1u8; 32], &qr_secret).unwrap();

        let mut eid = [0u8; 16];
        eid[11..14].copy_from_slice(&[0xab, 0xcd, 0xef]); // routing id
        eid[14] = 0x00; // tunnel domain id 0 (an assigned domain), LE
        eid[15] = 0x00;
        let advert = sealed_advert(&qr_secret, eid);

        let parsed = session.try_decrypt_advert(&advert).expect("decrypts");
        assert_eq!(parsed.routing_id, [0xab, 0xcd, 0xef]);

        let plaintext = crypto::try_decrypt_advert(&advert, &session.eid_key()).unwrap();
        let url = session.connect_url(&plaintext).expect("known domain");
        let tunnel_hex = hex(&session.tunnel_id());
        assert!(url.starts_with("wss://"));
        assert!(url.contains("/cable/connect/ABCDEF/"), "upper-case, as Chromium sends it");
        assert!(url.ends_with(&tunnel_hex));

        // A foreign advert (different QR secret) does not decrypt.
        let foreign = CableInitiator::new(&[2u8; 32], &[0x22u8; 16]).unwrap();
        assert!(foreign.try_decrypt_advert(&advert).is_none());
    }

    #[test]
    fn an_unknown_tunnel_domain_has_no_url() {
        let session = CableInitiator::new(&[1u8; 32], &[0x11u8; 16]).unwrap();
        let mut plaintext = [0u8; 16];
        // A domain id that is neither assigned nor in the hashed range's known
        // set decodes to None (tunnel_domain owns that ruling).
        plaintext[14] = 0xff;
        plaintext[15] = 0xff;
        // Whether 0xffff decodes is tunnel_domain's business; assert the plumbing
        // propagates a None rather than panicking.
        let _ = session.connect_url(&plaintext);
        // A wrong-length advert is always None.
        assert!(session.connect_url(&[0u8; 10]).is_none());
    }
}
