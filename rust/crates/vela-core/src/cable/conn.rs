//! The caBLE connection: run the Noise handshake over a shell-owned frame
//! transport, then carry CTAP2 over the encrypted channel as a [`Cable`] the
//! ceremony drives — the same interface the USB HID and CCID cables present, so
//! [`crate::ctap::ceremony`] cannot tell the phone apart from a plugged-in key.
//!
//! What is the CORE's (here): the Noise handshake, the transport framing (a
//! one-byte message type ‖ payload, sealed), reading the responder's
//! post-handshake `getInfo`, and turning a CTAP status into a [`CableError`].
//! What is the SHELL's (the [`CablePort`]): moving one frame each way over a
//! WebSocket tunnel or a BLE L2CAP channel, and saying which it is. Written once
//! so Android, iOS and the desktop share one definition of a caBLE exchange.
//!
//! Ported from the founder's proven demo (`HybridBleClient.kt` /
//! `HybridBleSession`), whose `sendCtap` is this module's [`Cable::exchange`].

use crate::ctap::ceremony::{Cable, CableError, TouchAnnouncer, TouchKind};
use crate::ctap::commands::split_response;
use crate::ctap::hid_cable::PortError;

use super::keys::KeyPair;
use super::noise::{CableTransport, NoiseError, NoiseInitiator};

/// The two hybrid data channels a caBLE session can run over, one binary frame
/// per message. The shell owns the radio and the socket; the core owns
/// everything the frames mean.
///
/// * WebSocket tunnel (CTAP 2.2): one binary WS frame per message.
/// * BLE L2CAP CoC (CTAP 2.3): a 4-byte big-endian length prefix per message.
///
/// Both reduce to "write these bytes as one message" / "read the next message",
/// which is this trait. A desktop `tokio-tungstenite` socket is one
/// implementation; an Android `BluetoothSocket` is another.
pub trait CablePort {
    /// Write one whole message. Framing (a WS binary frame, or a length prefix)
    /// is the port's; the bytes are the core's.
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), PortError>;
    /// Read the next whole message, blocking until one arrives or the transport
    /// gives up. [`PortError::TimedOut`] means the peer went silent.
    fn read_frame(&mut self) -> Result<Vec<u8>, PortError>;
    /// Which channel this is ("WebSocket" / "L2CAP") — for the diagnostics line
    /// and the PIN-cache identity, never shown as UI.
    fn channel(&self) -> &str;
}

/// The transport message type byte: `0x01` = a CTAP2 message, `0x00` = shutdown.
/// (caBLE also defines `0x02` update; this initiator neither sends nor needs
/// it.)
const MSG_CTAP: u8 = 0x01;
const MSG_SHUTDOWN: u8 = 0x00;

/// `authenticatorGetInfo` — a bare command byte, answered from the cache over
/// caBLE (see `cbor`).
const GET_INFO_COMMAND: u8 = 0x04;

/// How this module words a failure of the link to the other device — the
/// tunnel or BLE channel, and the handshake over it — as opposed to the
/// authenticator's answer. [`is_link_failure`] reads them back.
const TRANSPORT_FAILURE: &str = "caBLE transport: ";
const NOISE_FAILURE: &str = "caBLE Noise failure: ";

fn to_cable_error(error: NoiseError) -> CableError {
    CableError::Other(format!("{NOISE_FAILURE}{error:?}"))
}

fn port_to_cable_error(error: PortError) -> CableError {
    match error {
        PortError::TimedOut => CableError::TimedOut,
        PortError::WouldBlock => CableError::TimedOut,
        PortError::Io(detail) => CableError::Other(format!("{TRANSPORT_FAILURE}{detail}")),
    }
}

/// Whether a ceremony's failure detail is the link to the other device
/// failing rather than the authenticator: the sign-in and create sheets then
/// say to scan again, not to set up Face ID (issue #446, where an iPad's
/// tunnel was cancelled under it and the sheet blamed biometrics).
#[must_use]
pub fn is_link_failure(detail: &str) -> bool {
    detail.starts_with(TRANSPORT_FAILURE) || detail.starts_with(NOISE_FAILURE)
}

/// A connected caBLE session: the handshake is done, the transport cipher is
/// keyed, and the responder's `getInfo` has been read. Presents as a [`Cable`].
pub struct CableConnection<P: CablePort> {
    port: P,
    transport: CableTransport,
    /// The device's product string, for the sentence a failure shows and the
    /// "approve on your phone" prompt. caBLE has no product descriptor of its
    /// own, so the shell supplies a localised label.
    product: String,
    /// Stable identity for the PIN cache — the channel name. A caBLE peer is
    /// normally UV-on-phone (no client PIN), but the seam is uniform.
    path: String,
    /// The `authenticatorGetInfo` CBOR the responder volunteers right after the
    /// handshake, if any — the ceremony reads it instead of asking again.
    get_info: Option<Vec<u8>>,
    /// Fired once per exchange: on caBLE the approval happens on the PHONE, and
    /// there is no keepalive to learn it from, so the announcement goes out with
    /// the request — "look at your phone".
    on_touch: Option<TouchAnnouncer>,
}

impl<P: CablePort> CableConnection<P> {
    /// Run the KNpsk0 handshake over `port` and read the post-handshake message.
    ///
    /// * `static_seed` — the 32-byte seed of the keypair whose COMPRESSED public
    ///   key went into the QR (key 0). The shell keeps it from QR-build time and
    ///   hands it back here; the responder authenticates against it.
    /// * `ephemeral_seed` — 32 fresh bytes for this handshake's ephemeral key.
    /// * `psk` — [`super::crypto::psk`] of the QR secret and the decrypted BLE
    ///   advert plaintext.
    pub fn establish(
        mut port: P,
        static_seed: &[u8],
        ephemeral_seed: &[u8],
        psk: &[u8],
        product: String,
        on_touch: Option<TouchAnnouncer>,
    ) -> Result<Self, CableError> {
        let static_keys = KeyPair::from_seed(static_seed).ok_or_else(|| {
            CableError::Other("caBLE static seed is not a valid scalar".to_owned())
        })?;
        let ephemeral = KeyPair::from_seed(ephemeral_seed).ok_or_else(|| {
            CableError::Other("caBLE ephemeral seed is not a valid scalar".to_owned())
        })?;

        let mut noise = NoiseInitiator::new(static_keys);
        let msg1 = noise
            .write_message1(psk, ephemeral)
            .map_err(to_cable_error)?;
        port.write_frame(&msg1).map_err(port_to_cable_error)?;

        let msg2 = port.read_frame().map_err(port_to_cable_error)?;
        let (write_key, read_key) = noise.read_message2(&msg2).map_err(to_cable_error)?;
        let mut transport = CableTransport::new(write_key, read_key)
            .ok_or_else(|| CableError::Other("caBLE split produced a bad key".to_owned()))?;

        // The responder speaks first after the handshake: one encrypted frame
        // carrying (among other things) its getInfo at CBOR key 1.
        let first = port.read_frame().map_err(port_to_cable_error)?;
        let post = transport.open(&first).map_err(to_cable_error)?;
        let get_info = parse_post_handshake(&post);
        eprintln!(
            "[vela-cable] post-handshake {} bytes; getInfo cached: {}",
            post.len(),
            get_info.is_some()
        );

        let path = port.channel().to_owned();
        Ok(Self {
            port,
            transport,
            product,
            path,
            get_info,
            on_touch,
        })
    }

    /// The `getInfo` CBOR the responder volunteered, if it did.
    #[must_use]
    pub fn get_info(&self) -> Option<&[u8]> {
        self.get_info.as_deref()
    }

    /// One CTAP request → response over the sealed channel, before status
    /// interpretation. Mirrors the HID cable's `cbor`.
    fn cbor(&mut self, request: &[u8]) -> Result<Vec<u8>, CableError> {
        // `authenticatorGetInfo` is answered from the cache, never the wire.
        //
        // A caBLE responder VOLUNTEERS its getInfo in the post-handshake message
        // (read at `establish`), and a real one — iOS — is a one-shot: it runs
        // the single operation the QR hinted and closes the tunnel the instant it
        // receives any other command first. The shared ceremony opens with a
        // getInfo (the USB flow needs it to read versions and PIN protocols), so
        // without this that getInfo would be the first thing on the wire and the
        // phone would hang up before the operation ever ran. Returning the cached
        // copy makes the operation the first — and only — command the phone sees.
        if request.first() == Some(&GET_INFO_COMMAND) {
            if let Some(info) = &self.get_info {
                return Ok(info.clone());
            }
        }

        // A diagnostic line naming the CTAP command over the wire: the desktop's
        // one generic "sign-in failed" body cannot say whether a phone refused a
        // getAssertion (no passkey) or a makeCredential (create), and this is the
        // only place that knows.
        let command = match request.first() {
            Some(0x01) => "makeCredential",
            Some(0x02) => "getAssertion",
            Some(0x06) => "clientPin",
            Some(0x08) => "getNextAssertion",
            _ => "ctap",
        };
        eprintln!("[vela-cable] → CTAP {command} ({} bytes)", request.len());

        let mut frame = Vec::with_capacity(request.len() + 1);
        frame.push(MSG_CTAP);
        frame.extend_from_slice(request);
        let sealed = self.transport.seal(&frame).map_err(to_cable_error)?;
        self.port
            .write_frame(&sealed)
            .map_err(port_to_cable_error)?;

        let reply = self.port.read_frame().map_err(port_to_cable_error)?;
        let payload = self.transport.open(&reply).map_err(to_cable_error)?;
        let Some((&kind, body)) = payload.split_first() else {
            return Err(CableError::Other("empty caBLE frame".to_owned()));
        };
        if kind != MSG_CTAP {
            return Err(CableError::Other(format!(
                "non-CTAP caBLE frame (type {kind})"
            )));
        }
        let (status, response) = split_response(body)
            .map_err(|error| CableError::Other(format!("not a CTAP2 message: {error:?}")))?;
        if !status.is_success() {
            return Err(CableError::Ctap(status));
        }
        Ok(response.to_vec())
    }
}

impl<P: CablePort> Cable for CableConnection<P> {
    fn exchange(
        &mut self,
        request: &[u8],
        touch: Option<TouchKind>,
    ) -> Result<Vec<u8>, CableError> {
        // Announce up front: the phone shows its own approval prompt the moment
        // the request lands, and there is no keepalive to time the announcement
        // to — so it goes out with the request, not after one.
        if let (Some(kind), Some(callback)) = (touch, self.on_touch.as_mut()) {
            callback(kind, &self.product);
        }
        self.cbor(request)
    }

    fn cancel(&mut self) {
        // Best effort, as the trait promises: tell the responder to tear down.
        if let Ok(sealed) = self.transport.seal(&[MSG_SHUTDOWN]) {
            let _ = self.port.write_frame(&sealed);
        }
    }

    fn product(&self) -> &str {
        &self.product
    }

    fn path(&self) -> &str {
        &self.path
    }
}

/// The responder's post-handshake message carries its `getInfo` bytes at CBOR
/// map key 1. Anything else (a malformed frame, a missing key) is simply "no
/// pre-supplied getInfo" — the ceremony then asks for it over the channel.
fn parse_post_handshake(payload: &[u8]) -> Option<Vec<u8>> {
    use ciborium::Value;
    let value: Value = ciborium::de::from_reader(payload).ok()?;
    let Value::Map(entries) = value else {
        return None;
    };
    for (key, val) in entries {
        if matches!(key, Value::Integer(i) if i == 1.into()) {
            if let Value::Bytes(bytes) = val {
                return Some(bytes);
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::cable::noise::CableTransport as Transport;
    use crate::ctap::ceremony::TouchKind;
    use crate::ctap::commands::Status;
    use std::collections::VecDeque;

    /// An in-memory responder port: it runs the OTHER half of the handshake and
    /// then answers CTAP frames from a scripted queue, so the connection is
    /// exercised over a real Noise channel (not a stub cipher).
    struct LoopbackPort {
        inbox: VecDeque<Vec<u8>>,
        // The responder's transport, once the handshake is done.
        transport: Option<Transport>,
        // Queued CTAP response payloads (status ‖ cbor) to seal on each request.
        ctap_replies: VecDeque<Vec<u8>>,
        // Captured plaintext CTAP requests the initiator sent (type byte stripped).
        sent_requests: Vec<Vec<u8>>,
        // Handshake state driven from the test harness before establish().
        handshake: HandshakeScript,
    }

    /// The responder side of the handshake, precomputed by the harness so the
    /// port can answer `write_frame(msg1)` with `msg2` and then the encrypted
    /// post-handshake frame.
    struct HandshakeScript {
        responder: Option<super::super::noise::testonly::Responder>,
        psk: Vec<u8>,
        post_handshake_cbor: Vec<u8>,
    }

    impl CablePort for LoopbackPort {
        fn write_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
            if let Some(mut responder) = self.handshake.responder.take() {
                // This is message 1; produce message 2 + the post-handshake frame.
                let (msg2, mut transport) = responder.respond(frame, &self.handshake.psk);
                self.inbox.push_back(msg2);
                let sealed = transport.seal(&self.handshake.post_handshake_cbor).unwrap();
                self.inbox.push_back(sealed);
                self.transport = Some(transport);
                return Ok(());
            }
            // A post-handshake CTAP request: open it, record it, seal a reply.
            let transport = self.transport.as_mut().expect("handshake done");
            let pt = transport.open(frame).unwrap();
            assert_eq!(pt.first(), Some(&MSG_CTAP));
            self.sent_requests.push(pt[1..].to_vec());
            let reply_body = self.ctap_replies.pop_front().expect("a scripted reply");
            let mut framed = vec![MSG_CTAP];
            framed.extend_from_slice(&reply_body);
            let sealed = transport.seal(&framed).unwrap();
            self.inbox.push_back(sealed);
            Ok(())
        }

        fn read_frame(&mut self) -> Result<Vec<u8>, PortError> {
            self.inbox.pop_front().ok_or(PortError::TimedOut)
        }

        fn channel(&self) -> &str {
            "Loopback"
        }
    }

    fn ctap_ok(cbor: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00u8]; // CTAP2_OK
        v.extend_from_slice(cbor);
        v
    }

    #[test]
    fn establish_runs_the_handshake_and_reads_get_info() {
        let psk = vec![0x33u8; 32];
        let init_static_seed = [1u8; 32];
        let static_pub = KeyPair::from_seed(&init_static_seed)
            .unwrap()
            .public_uncompressed();

        // Post-handshake: a CBOR map {1: <getInfo bytes>}.
        let get_info = vec![0xa1, 0x01, 0x42, 0xde, 0xad]; // {1: h'dead'}
        let responder = super::super::noise::testonly::Responder::new(
            static_pub,
            KeyPair::from_seed(&[3u8; 32]).unwrap(),
        );
        let port = LoopbackPort {
            inbox: VecDeque::new(),
            transport: None,
            ctap_replies: VecDeque::from(vec![ctap_ok(&[0xa0])]), // a getAssertion-ish reply
            sent_requests: Vec::new(),
            handshake: HandshakeScript {
                responder: Some(responder),
                psk: psk.clone(),
                post_handshake_cbor: get_info.clone(),
            },
        };

        let mut conn = CableConnection::establish(
            port,
            &init_static_seed,
            &[2u8; 32],
            &psk,
            "iPhone".to_owned(),
            None,
        )
        .expect("handshake");

        assert_eq!(conn.get_info(), Some(&[0xde, 0xad][..]));
        assert_eq!(conn.product(), "iPhone");

        // authenticatorGetInfo (0x04) is answered from the post-handshake cache,
        // NOT the wire — the single scripted reply below is left for the real
        // operation, proving no frame went out for the getInfo. (A real caBLE
        // phone closes the tunnel if getInfo is the first thing it receives.)
        let info = conn.exchange(&[0x04], None).unwrap();
        assert_eq!(info, vec![0xde, 0xad]);

        // A CTAP exchange round-trips the encrypted channel and strips the
        // status byte, exactly like the HID/CCID cables.
        let body = conn
            .exchange(&[0x02, 0x11], Some(TouchKind::Presence))
            .unwrap();
        assert_eq!(body, vec![0xa0]);
    }

    #[test]
    fn a_ctap_error_status_becomes_a_ctap_error() {
        let psk = vec![0x33u8; 32];
        let init_static_seed = [1u8; 32];
        let static_pub = KeyPair::from_seed(&init_static_seed)
            .unwrap()
            .public_uncompressed();
        let responder = super::super::noise::testonly::Responder::new(
            static_pub,
            KeyPair::from_seed(&[3u8; 32]).unwrap(),
        );
        // 0x2e = CTAP2_ERR_NO_CREDENTIALS.
        let port = LoopbackPort {
            inbox: VecDeque::new(),
            transport: None,
            ctap_replies: VecDeque::from(vec![vec![0x2e]]),
            sent_requests: Vec::new(),
            handshake: HandshakeScript {
                responder: Some(responder),
                psk: psk.clone(),
                post_handshake_cbor: vec![0xa0], // empty map, no getInfo
            },
        };
        let mut conn = CableConnection::establish(
            port,
            &init_static_seed,
            &[2u8; 32],
            &psk,
            "phone".to_owned(),
            None,
        )
        .unwrap();
        assert_eq!(conn.get_info(), None);
        match conn.exchange(&[0x02], None) {
            Err(CableError::Ctap(Status::NoCredentials)) => {}
            other => panic!("expected NoCredentials, got {other:?}"),
        }
    }
}
