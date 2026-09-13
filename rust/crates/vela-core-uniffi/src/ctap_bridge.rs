//! The app-owned CTAP2 ceremony, exported to Kotlin (and Swift).
//!
//! The whole protocol — CTAPHID framing, the PIN/UV dance, `getNextAssertion`
//! enumeration, what a status byte means — lives in `vela_core::ctap` and runs
//! HERE, in Rust. The shell provides only two things across the FFI, and both
//! are pure platform work the core has no business doing:
//!
//! * [`UsbHidPort`] — move 64 bytes each way over `android.hardware.usb`, and
//!   own the read clock. A Kotlin object implements it.
//! * [`CtapCeremonyHost`] — the PIN dialog, the which-wallet picker, the
//!   platform CSPRNG, and one diagnostics line. A Kotlin object implements it.
//!
//! So an OEM whose system passkey sheet cannot reach a security key — a
//! GrapheneOS or CalyxOS phone, or any China-market device without full GMS —
//! creates and signs into a wallet with a hardware key anyway: no Google
//! service, no domain association, no OEM sheet. The ceremony that runs is
//! byte-for-byte the one the desktop runs, because it is the same code.

use std::sync::Arc;

use vela_core::ctap::apdu_cable::{ApduCable, ApduError, ApduPort};
use vela_core::ctap::ceremony::{self, CredentialChoice, Host, PinRequest, TouchKind};
use vela_core::ctap::hid::HID_REPORT_SIZE;
use vela_core::ctap::hid_cable::{HidCable, HidPort, PortError};

/// The relying party every Vela passkey is bound to — the same string the web
/// and desktop clients use. A passkey cannot move between relying parties, so
/// this is part of the wallet's identity: it lives in the bridge, not in the
/// shell, so Kotlin cannot get it wrong.
const RELYING_PARTY: &str = "getvela.app";
const RELYING_PARTY_NAME: &str = "Vela Wallet";
const ORIGIN: &str = "https://getvela.app";

// ---------------------------------------------------------------------------
// The two seams, as Kotlin sees them
// ---------------------------------------------------------------------------

/// One 64-byte-report USB conversation with one security key. Kotlin owns the
/// endpoints and the read timeout; the framing above it is the core's.
#[uniffi::export(with_foreign)]
pub trait UsbHidPort: Send + Sync {
    /// Write one 64-byte report (no report-id byte — Android's bulk transfer
    /// takes the raw HID packet). `Some(detail)` is a write failure.
    fn write_report(&self, report: Vec<u8>) -> Option<String>;
    /// Read one report, blocking up to the port's own read slice.
    fn read_report(&self) -> HidReadOutcome;
    fn product(&self) -> String;
    fn path(&self) -> String;
}

/// What one [`UsbHidPort::read_report`] produced. Not a `Result`, because
/// "nothing arrived in this slice" is the common case in the poll loop and must
/// not cost a thrown exception on every idle read.
#[derive(uniffi::Enum)]
pub enum HidReadOutcome {
    /// A full 64-byte report.
    Report { bytes: Vec<u8> },
    /// The read slice elapsed with nothing to read. The cable loops.
    WouldBlock,
    /// The overall exchange budget is spent — the key stopped answering.
    TimedOut,
    /// The transport failed in its own words.
    Failed { detail: String },
}

/// The person and the platform, as the ceremony sees them.
#[uniffi::export(with_foreign)]
pub trait CtapCeremonyHost: Send + Sync {
    /// Ask for the key's PIN. `None` is a dismissal (a cancellation).
    fn pin(&self, request: CtapPinRequest) -> Option<String>;
    /// One key answered for several wallets — which? `None` is a dismissal.
    fn pick(&self, choices: Vec<CtapCredentialChoice>) -> Option<u32>;
    /// Platform CSPRNG. Every value is a challenge, an IV or an ephemeral key.
    fn random(&self, len: u32) -> Vec<u8>;
    /// One diagnostics line (the `getInfo` summary) — VelaLog on Android.
    fn note(&self, line: String);
    /// The key is blinking and waiting for a finger or a button. `kind` is one
    /// of "presence" / "fingerprint" / "select".
    fn touch(&self, kind: String, product: String);
}

/// What the PIN dialog needs to say.
#[derive(uniffi::Record)]
pub struct CtapPinRequest {
    pub product: String,
    /// The device identity, so a PIN cache never hands one key's PIN to
    /// another. Not for display.
    pub device: String,
    /// Attempts left, when the key would say (`-1` when it would not — uniffi
    /// has no bare optional in a callback arg's ergonomics, so absence is the
    /// sentinel the Kotlin side already reads as "unknown").
    pub retries: i32,
    /// A previous attempt in this session was refused.
    pub retry: bool,
}

/// One wallet a key holds, for the picker.
#[derive(uniffi::Record)]
pub struct CtapCredentialChoice {
    pub name: String,
    pub credential_id: String,
    pub product: String,
}

// ---------------------------------------------------------------------------
// Results and failure
// ---------------------------------------------------------------------------

/// A completed registration, in the core's hex vocabulary. The attachment and
/// transports are what the USB path is by construction — a removable key.
#[derive(uniffi::Record)]
pub struct CtapRegistration {
    pub credential_id_hex: String,
    pub attestation_object_hex: String,
    pub client_data_json_hex: String,
    pub authenticator_attachment: String,
    pub transports: String,
}

/// A completed assertion.
#[derive(uniffi::Record)]
pub struct CtapAssertion {
    pub credential_id_hex: String,
    pub signature_der_hex: String,
    pub authenticator_data_hex: String,
    pub client_data_json_hex: String,
    /// Absent (empty) is a different fact from a present-but-empty handle; the
    /// core's name resolution branches on it. Empty here means absent.
    pub user_id_hex: String,
    pub authenticator_attachment: String,
}

/// A ceremony that produced no credential, already classified. `kind` is the
/// same four-way vocabulary the crux machines branch on
/// (`cancelled` / `not_supported` / `not_discoverable` / `other`), so the
/// Android executor forwards it as the shell result the core expects.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CtapError {
    #[error("cancelled")]
    Cancelled,
    #[error("{detail}")]
    NotSupported { detail: String },
    #[error("{detail}")]
    NotDiscoverable { detail: String },
    #[error("{detail}")]
    Other { detail: String },
}

impl From<ceremony::CeremonyError> for CtapError {
    fn from(error: ceremony::CeremonyError) -> Self {
        let detail = error.message.unwrap_or_default();
        match error.kind {
            ceremony::FailureKind::Cancelled => CtapError::Cancelled,
            ceremony::FailureKind::NotSupported => CtapError::NotSupported { detail },
            ceremony::FailureKind::NotDiscoverable => CtapError::NotDiscoverable { detail },
            ceremony::FailureKind::Other => CtapError::Other { detail },
        }
    }
}

// ---------------------------------------------------------------------------
// Adapters: the FFI callbacks, wearing the core's traits
// ---------------------------------------------------------------------------

/// The Kotlin port, as the core's [`HidPort`]. The `&mut self` the core asks
/// for delegates to the callback's `&self`; the Kotlin object owns whatever
/// mutability it needs. `product`/`path` are captured at open, because the
/// core wants a borrow and the callback returns owned strings.
struct PortAdapter {
    inner: Arc<dyn UsbHidPort>,
    product: String,
    path: String,
}

impl HidPort for PortAdapter {
    fn write_report(&mut self, report: &[u8; HID_REPORT_SIZE]) -> Result<(), PortError> {
        match self.inner.write_report(report.to_vec()) {
            None => Ok(()),
            Some(detail) => Err(PortError::Io(detail)),
        }
    }

    fn read_report(&mut self) -> Result<[u8; HID_REPORT_SIZE], PortError> {
        match self.inner.read_report() {
            HidReadOutcome::Report { bytes } => {
                let array: [u8; HID_REPORT_SIZE] = bytes
                    .try_into()
                    .map_err(|_| PortError::Io("USB report was not 64 bytes".to_owned()))?;
                Ok(array)
            }
            HidReadOutcome::WouldBlock => Err(PortError::WouldBlock),
            HidReadOutcome::TimedOut => Err(PortError::TimedOut),
            HidReadOutcome::Failed { detail } => Err(PortError::Io(detail)),
        }
    }

    fn product(&self) -> &str {
        // The core wants a borrow; the callback returns an owned string. The
        // product is read once per get_info line and per failure sentence, so
        // caching it at open avoids a callback per borrow.
        &self.product
    }

    fn path(&self) -> &str {
        &self.path
    }
}

/// The Kotlin host, as the core's [`Host`].
struct HostAdapter {
    inner: Arc<dyn CtapCeremonyHost>,
}

impl Host for HostAdapter {
    fn pin(&self, request: PinRequest) -> Option<String> {
        self.inner.pin(CtapPinRequest {
            product: request.product,
            device: request.device,
            retries: request.retries.map(|r| r as i32).unwrap_or(-1),
            retry: request.retry,
        })
    }

    fn pick(&self, choices: Vec<CredentialChoice>) -> Option<usize> {
        let mapped = choices
            .into_iter()
            .map(|choice| CtapCredentialChoice {
                name: choice.name,
                credential_id: choice.credential_id,
                product: choice.product,
            })
            .collect();
        self.inner.pick(mapped).map(|index| index as usize)
    }

    fn random(&self, len: usize) -> Vec<u8> {
        self.inner.random(len as u32)
    }

    fn note(&self, line: &str) {
        self.inner.note(line.to_owned());
    }
}

// ---------------------------------------------------------------------------
// The exported ceremonies
// ---------------------------------------------------------------------------

/// Open the cable: INIT with a nonce the host supplies, wired to announce the
/// touch moment through the host. Shared by register and assert.
fn open_cable(
    port: Arc<dyn UsbHidPort>,
    host: Arc<dyn CtapCeremonyHost>,
) -> Result<HidCable<PortAdapter>, CtapError> {
    let adapter = PortAdapter {
        product: port.product(),
        path: port.path(),
        inner: port,
    };
    let nonce: [u8; 8] = host.random(8).try_into().map_err(|_| CtapError::Other {
        detail: "the host CSPRNG did not return 8 bytes".to_owned(),
    })?;
    let mut cable = HidCable::open(adapter, nonce)
        .map_err(|error| CtapError::from(ceremony::failure_for(error)))?;
    let touch_host = Arc::clone(&host);
    cable.on_touch(Box::new(move |kind, product| {
        let name = match kind {
            TouchKind::Presence => "presence",
            TouchKind::Fingerprint => "fingerprint",
            TouchKind::Select => "select",
        };
        touch_host.touch(name.to_owned(), product.to_owned());
    }));
    Ok(cable)
}

/// `RegisterPasskey` over USB: mint a founding key on the plugged-in security
/// key. The touch, the PIN, the discoverability gate — all the core's; this
/// only carries the bytes and the prompts.
#[uniffi::export]
pub fn ctap_register(
    port: Arc<dyn UsbHidPort>,
    host: Arc<dyn CtapCeremonyHost>,
    name: String,
    exclude_credential_ids: Vec<String>,
) -> Result<CtapRegistration, CtapError> {
    let mut cable = open_cable(port, Arc::clone(&host))?;
    let host_adapter = HostAdapter { inner: host };
    let registration = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(&name, &exclude_credential_ids)?;

    Ok(CtapRegistration {
        credential_id_hex: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        authenticator_attachment: "cross-platform".to_owned(),
        transports: "usb".to_owned(),
    })
}

/// `SignProof` / `SignMemberProof` / `AuthenticatePasskey` over USB: one
/// assertion. `credential_id_hex` empty is the "who are you?" sign-in ceremony.
#[uniffi::export]
pub fn ctap_assert(
    port: Arc<dyn UsbHidPort>,
    host: Arc<dyn CtapCeremonyHost>,
    challenge: Vec<u8>,
    credential_id_hex: String,
) -> Result<CtapAssertion, CtapError> {
    let mut cable = open_cable(port, Arc::clone(&host))?;
    let host_adapter = HostAdapter { inner: host };
    let pinned = if credential_id_hex.is_empty() {
        None
    } else {
        Some(credential_id_hex.as_str())
    };
    let assertion = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(&challenge, pinned)?;

    Ok(CtapAssertion {
        credential_id_hex: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex.unwrap_or_default(),
        authenticator_attachment: "cross-platform".to_owned(),
    })
}

// ---------------------------------------------------------------------------
// CCID / NFC — the APDU transport (iOS TKSmartCard, iOS/Android IsoDep)
// ---------------------------------------------------------------------------
//
// The same ceremony over the other FIDO binding: CTAP2 in ISO 7816 APDUs. The
// core's `ApduCable` owns the applet SELECT, the keepalive poll and the GET
// RESPONSE chaining; the shell owns only one call — transmit a command APDU,
// get its response bytes and status word back. iOS reaches a USB-C security key
// this way (CryptoTokenKit has no HID host, but it has a smart-card interface),
// and the same port serves NFC on both platforms.

/// One APDU exchange with one card. Kotlin/Swift owns the reader; the framing
/// above it is the core's.
#[uniffi::export(with_foreign)]
pub trait CcidPort: Send + Sync {
    /// Transmit one command APDU; return the full response INCLUDING the two
    /// trailing status-word bytes. No 61xx chaining or keepalive handling here.
    fn transmit(&self, apdu: Vec<u8>) -> ApduOutcome;
    /// Sleep the transport's keepalive poll interval (~100 ms). Called only
    /// between `0x9100` keepalives.
    fn poll_delay(&self);
    fn product(&self) -> String;
    fn path(&self) -> String;
}

/// What one [`CcidPort::transmit`] produced.
#[derive(uniffi::Enum)]
pub enum ApduOutcome {
    /// The response APDU, data followed by the two status-word bytes.
    Response { bytes: Vec<u8> },
    /// No reader or card is present.
    NoCard,
    /// The transport failed in its own words.
    Failed { detail: String },
}

/// The Kotlin/Swift card, as the core's [`ApduPort`].
struct CcidPortAdapter {
    inner: Arc<dyn CcidPort>,
    product: String,
    path: String,
}

impl ApduPort for CcidPortAdapter {
    fn transmit(&mut self, apdu: &[u8]) -> Result<Vec<u8>, ApduError> {
        match self.inner.transmit(apdu.to_vec()) {
            ApduOutcome::Response { bytes } => Ok(bytes),
            ApduOutcome::NoCard => Err(ApduError::NoCard),
            ApduOutcome::Failed { detail } => Err(ApduError::Io(detail)),
        }
    }

    fn poll_delay(&mut self) {
        self.inner.poll_delay();
    }

    fn product(&self) -> &str {
        &self.product
    }

    fn path(&self) -> &str {
        &self.path
    }
}

/// Open the APDU cable (SELECT the FIDO applet), wired to announce the touch
/// moment through the host. Shared by the two CCID ceremonies.
fn open_apdu_cable(
    port: Arc<dyn CcidPort>,
    host: Arc<dyn CtapCeremonyHost>,
) -> Result<ApduCable<CcidPortAdapter>, CtapError> {
    let adapter = CcidPortAdapter {
        product: port.product(),
        path: port.path(),
        inner: port,
    };
    let mut cable =
        ApduCable::open(adapter).map_err(|error| CtapError::from(ceremony::failure_for(error)))?;
    let touch_host = Arc::clone(&host);
    cable.on_touch(Box::new(move |kind, product| {
        let name = match kind {
            TouchKind::Presence => "presence",
            TouchKind::Fingerprint => "fingerprint",
            TouchKind::Select => "select",
        };
        touch_host.touch(name.to_owned(), product.to_owned());
    }));
    Ok(cable)
}

/// `RegisterPasskey` over CCID/NFC: mint a founding key on the presented card.
#[uniffi::export]
pub fn ctap_register_ccid(
    port: Arc<dyn CcidPort>,
    host: Arc<dyn CtapCeremonyHost>,
    name: String,
    exclude_credential_ids: Vec<String>,
) -> Result<CtapRegistration, CtapError> {
    let mut cable = open_apdu_cable(port, Arc::clone(&host))?;
    let host_adapter = HostAdapter { inner: host };
    let registration = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(&name, &exclude_credential_ids)?;

    Ok(CtapRegistration {
        credential_id_hex: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        // The card path reports NFC alongside USB — a CCID key is the same key
        // that answers over NFC, and both are removable transports.
        authenticator_attachment: "cross-platform".to_owned(),
        transports: "usb,nfc".to_owned(),
    })
}

/// One assertion over CCID/NFC. `credential_id_hex` empty is sign-in.
#[uniffi::export]
pub fn ctap_assert_ccid(
    port: Arc<dyn CcidPort>,
    host: Arc<dyn CtapCeremonyHost>,
    challenge: Vec<u8>,
    credential_id_hex: String,
) -> Result<CtapAssertion, CtapError> {
    let mut cable = open_apdu_cable(port, Arc::clone(&host))?;
    let host_adapter = HostAdapter { inner: host };
    let pinned = if credential_id_hex.is_empty() {
        None
    } else {
        Some(credential_id_hex.as_str())
    };
    let assertion = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(&challenge, pinned)?;

    Ok(CtapAssertion {
        credential_id_hex: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex.unwrap_or_default(),
        authenticator_attachment: "cross-platform".to_owned(),
    })
}

// ---------------------------------------------------------------------------
// caBLE v2 — "sign in with your phone" (the hybrid transport)
// ---------------------------------------------------------------------------
//
// The QR, the BLE advert decrypt, the Noise handshake and the CTAP-over-Noise
// framing are ALL the core's (`vela_core::cable`). The shell provides only the
// radio and the socket: it scans for the advert this QR names, opens the
// WebSocket tunnel, and moves one frame each way. The ceremony that runs over
// the encrypted channel is byte-for-byte the USB/CCID one — a phone reached by
// caBLE is just another `Cable`.

use vela_core::cable::conn::{CableConnection, CablePort as CoreCablePort};
use vela_core::cable::crypto as cable_crypto;
use vela_core::cable::session::CableInitiator;
use vela_core::ctap::ceremony::TouchAnnouncer;

/// One message-oriented caBLE frame transport, as Kotlin/Swift sees it. The
/// shell owns the WebSocket (or BLE L2CAP) socket; the Noise handshake and the
/// CTAP framing above it are the core's.
#[uniffi::export(with_foreign)]
pub trait CableFramePort: Send + Sync {
    /// Write one whole message (a WS binary frame, or a length-prefixed L2CAP
    /// message — the port decides). `Some(detail)` is a failure.
    fn write_frame(&self, frame: Vec<u8>) -> Option<String>;
    /// Read the next whole message, blocking until one arrives or the transport
    /// gives up.
    fn read_frame(&self) -> CableFrameOutcome;
    /// "WebSocket" / "L2CAP" — diagnostics and the PIN-cache identity, never UI.
    fn channel(&self) -> String;
}

/// What one [`CableFramePort::read_frame`] produced.
#[derive(uniffi::Enum)]
pub enum CableFrameOutcome {
    /// One whole message.
    Frame { bytes: Vec<u8> },
    /// The peer went silent.
    TimedOut,
    /// The transport failed in its own words.
    Failed { detail: String },
}

/// The parsed BLE proximity advert that named this QR's phone.
#[derive(uniffi::Record)]
pub struct CableAdvert {
    /// The 16-byte decrypted EID plaintext — fed back to `cable_connect_url` and
    /// the assert/register calls (the PSK derives from it).
    pub plaintext: Vec<u8>,
    /// The 3-byte tunnel routing id (part of the WebSocket URL).
    pub routing_id: Vec<u8>,
    /// The tunnel-server domain id.
    pub tunnel_domain_id: u16,
    /// The L2CAP PSM from the advert's CTAP 2.3 BLE suffix, when the
    /// authenticator offers the direct BLE channel. `Some` is the shell's cue
    /// to connect an L2CAP CoC to this port on the advertising peripheral — no
    /// tunnel, no internet; `None` means WebSocket-only (GMS-era phones).
    pub psm: Option<u16>,
}

/// The Kotlin/Swift port, as the core's [`CoreCablePort`].
struct CablePortAdapter {
    inner: Arc<dyn CableFramePort>,
    channel: String,
}

impl CoreCablePort for CablePortAdapter {
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        match self.inner.write_frame(frame.to_vec()) {
            None => Ok(()),
            Some(detail) => Err(PortError::Io(detail)),
        }
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, PortError> {
        match self.inner.read_frame() {
            CableFrameOutcome::Frame { bytes } => Ok(bytes),
            CableFrameOutcome::TimedOut => Err(PortError::TimedOut),
            CableFrameOutcome::Failed { detail } => Err(PortError::Io(detail)),
        }
    }

    fn channel(&self) -> &str {
        &self.channel
    }
}

/// The `FIDO:/…` QR payload to render. `None` if the secrets are malformed
/// (wrong length, or a static seed that is not a valid scalar — the shell
/// retries with fresh randomness).
#[uniffi::export]
pub fn cable_qr_payload(
    static_seed: Vec<u8>,
    qr_secret: Vec<u8>,
    epoch_seconds: i64,
    for_get: bool,
) -> Option<String> {
    CableInitiator::new(&static_seed, &qr_secret)
        .map(|session| session.qr_payload(epoch_seconds, for_get))
}

/// The 64-byte advert key (AES-256 ‖ HMAC-SHA256) for a shell that trial-decrypts
/// BLE adverts in native code rather than through [`cable_try_decrypt_advert`].
#[uniffi::export]
pub fn cable_eid_key(qr_secret: Vec<u8>) -> Vec<u8> {
    cable_crypto::eid_key(&qr_secret)
}

/// A QR code as a square module matrix, row-major, `true` = dark. The same
/// encoder the desktop draws with, so every platform renders the same code for
/// the same payload; the shells own only pixels.
#[derive(uniffi::Record)]
pub struct QrMatrix {
    /// Modules per side.
    pub width: u32,
    /// `width * width` booleans, row-major.
    pub modules: Vec<bool>,
}

/// Encode any text as a QR matrix. `None` only when the text cannot fit a QR
/// code at all.
///
/// The receive screen draws a wallet address with this, and the caBLE flow
/// draws a `FIDO:/…` payload with it. Both are "a string somebody points a
/// camera at", and both must be the SAME encoder as the other platforms — a
/// wallet whose QR is drawn by four different encoders is a wallet where one
/// platform's code scans and another's does not.
#[uniffi::export]
pub fn qr_matrix(text: String) -> Option<QrMatrix> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let width = u32::try_from(code.width()).ok()?;
    Some(QrMatrix {
        width,
        modules: code
            .to_colors()
            .into_iter()
            .map(|color| color == qrcode::Color::Dark)
            .collect(),
    })
}

/// The caBLE flow's name for [`qr_matrix`], kept so onboarding does not move.
#[uniffi::export]
pub fn cable_qr_matrix(text: String) -> Option<QrMatrix> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let width = u32::try_from(code.width()).ok()?;
    Some(QrMatrix {
        width,
        modules: code
            .to_colors()
            .into_iter()
            .map(|color| color == qrcode::Color::Dark)
            .collect(),
    })
}

/// Trial-decrypt one BLE advert candidate; `Some` when it is this QR's phone.
/// Pure crypto — needs only the QR secret.
///
/// Pass the WHOLE service-data payload: the first 20 bytes are the sealed EID,
/// and anything after them is the CTAP 2.3 BLE suffix — a CBOR map whose key 1
/// is the L2CAP PSM. Handing the full payload here is what lets the shell get
/// `psm` back without owning any CBOR of its own.
#[uniffi::export]
pub fn cable_try_decrypt_advert(qr_secret: Vec<u8>, candidate: Vec<u8>) -> Option<CableAdvert> {
    if candidate.len() < 20 {
        return None;
    }
    let key = cable_crypto::eid_key(&qr_secret);
    let plaintext = cable_crypto::try_decrypt_advert(&candidate[0..20], &key)?;
    let eid = cable_crypto::AdvertEid::parse(&plaintext)?;
    Some(CableAdvert {
        plaintext: plaintext.to_vec(),
        routing_id: eid.routing_id.to_vec(),
        tunnel_domain_id: eid.tunnel_domain_id,
        psm: cable_crypto::parse_advert_psm(&candidate[20..]),
    })
}

/// The WebSocket tunnel URL to open, from the decrypted advert plaintext. `None`
/// if the secrets or the advert are malformed, or the advert names an unknown
/// tunnel domain.
#[uniffi::export]
pub fn cable_connect_url(
    static_seed: Vec<u8>,
    qr_secret: Vec<u8>,
    advert_plaintext: Vec<u8>,
) -> Option<String> {
    CableInitiator::new(&static_seed, &qr_secret)?.connect_url(&advert_plaintext)
}

/// Open the caBLE cable: run the Noise handshake over the shell's connected
/// socket, wired to announce the touch (which happens ON THE PHONE) through the
/// host. Shared by register and assert.
fn establish_cable(
    port: Arc<dyn CableFramePort>,
    host: Arc<dyn CtapCeremonyHost>,
    static_seed: Vec<u8>,
    qr_secret: Vec<u8>,
    advert_plaintext: Vec<u8>,
    product: String,
) -> Result<CableConnection<CablePortAdapter>, CtapError> {
    let session =
        CableInitiator::new(&static_seed, &qr_secret).ok_or_else(|| CtapError::Other {
            detail: "the caBLE session secrets are malformed".to_owned(),
        })?;
    let adapter = CablePortAdapter {
        channel: port.channel(),
        inner: port,
    };
    let ephemeral_seed = host.random(32);
    let touch_host = Arc::clone(&host);
    let on_touch: TouchAnnouncer = Box::new(move |kind, product| {
        let name = match kind {
            TouchKind::Presence => "presence",
            TouchKind::Fingerprint => "fingerprint",
            TouchKind::Select => "select",
        };
        touch_host.touch(name.to_owned(), product.to_owned());
    });
    session
        .establish(
            adapter,
            &advert_plaintext,
            &ephemeral_seed,
            product,
            Some(on_touch),
        )
        .map_err(|error| CtapError::from(ceremony::failure_for(error)))
}

/// `RegisterPasskey` over caBLE: mint a founding key that lives on the phone.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn ctap_register_cable(
    port: Arc<dyn CableFramePort>,
    host: Arc<dyn CtapCeremonyHost>,
    static_seed: Vec<u8>,
    qr_secret: Vec<u8>,
    advert_plaintext: Vec<u8>,
    product: String,
    name: String,
    exclude_credential_ids: Vec<String>,
) -> Result<CtapRegistration, CtapError> {
    let mut cable = establish_cable(
        port,
        Arc::clone(&host),
        static_seed,
        qr_secret,
        advert_plaintext,
        product,
    )?;
    let host_adapter = HostAdapter { inner: host };
    let registration = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(&name, &exclude_credential_ids)?;
    // See `ctap_assert_cable`: the shutdown frame is the polite goodbye.
    vela_core::ctap::ceremony::Cable::cancel(&mut cable);

    Ok(CtapRegistration {
        credential_id_hex: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        authenticator_attachment: "cross-platform".to_owned(),
        // The phone is reached over the hybrid transport.
        transports: "hybrid".to_owned(),
    })
}

/// One assertion over caBLE. `credential_id_hex` empty is the "who are you?"
/// sign-in ceremony (the phone offers whatever Vela passkeys it holds).
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn ctap_assert_cable(
    port: Arc<dyn CableFramePort>,
    host: Arc<dyn CtapCeremonyHost>,
    static_seed: Vec<u8>,
    qr_secret: Vec<u8>,
    advert_plaintext: Vec<u8>,
    product: String,
    challenge: Vec<u8>,
    credential_id_hex: String,
) -> Result<CtapAssertion, CtapError> {
    let mut cable = establish_cable(
        port,
        Arc::clone(&host),
        static_seed,
        qr_secret,
        advert_plaintext,
        product,
    )?;
    let host_adapter = HostAdapter { inner: host };
    let pinned = if credential_id_hex.is_empty() {
        None
    } else {
        Some(credential_id_hex.as_str())
    };
    let assertion = ceremony::Client {
        cable: &mut cable,
        host: &host_adapter,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(&challenge, pinned)?;
    // Say goodbye before the shell drops the socket: the caBLE shutdown frame
    // lets the phone end its session loop cleanly instead of decrypting the
    // transport teardown as a garbled frame (a BAD_DECRYPT in its log after
    // every success — observed live against the securitykeys authenticator).
    vela_core::ctap::ceremony::Cable::cancel(&mut cable);

    Ok(CtapAssertion {
        credential_id_hex: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex.unwrap_or_default(),
        authenticator_attachment: "cross-platform".to_owned(),
    })
}
