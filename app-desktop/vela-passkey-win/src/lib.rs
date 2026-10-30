//! The Windows WebAuthn API — the one place the wallet asks an operating
//! system to run a passkey ceremony on its behalf.
//!
//! ## Why this exists at all
//!
//! Everywhere else, the desktop client talks to security keys itself, because
//! no desktop offers it a passkey service. Windows is the exception, and not by
//! preference: **since Windows 10 build 1903 a non-elevated process cannot open
//! a FIDO HID device.** The OS reserves them for `webauthn.dll`. That is why
//! `libfido2` uses this same API on Windows rather than its own HID backend,
//! and why `fido-key-manager` ships an elevation manifest — asking someone to
//! run their wallet as Administrator is not an answer.
//!
//! So for a key **on the USB port**, the platform's CTAP client is the only
//! route. Everything `ctap/usb.rs` does on macOS and Linux — choosing a device,
//! the touch prompt, the PIN prompt, enumerating credentials, the picker —
//! Windows does itself, in its own dialog. That is a smaller surface, not a
//! bigger one.
//!
//! ## What this crate is NOT for
//!
//! **The phone.** The lockdown is on FIDO HID devices, and the hybrid (caBLE)
//! transport touches none: it is a BLE advertisement scan and a WebSocket, both
//! of which a non-elevated process may do. So `ctap/cable.rs` runs on Windows
//! exactly as it does on the other two desktops, and `KeyMethod::Hybrid` never
//! reaches this crate. That matters beyond tidiness — `webauthn.dll` only grew
//! its own QR flow in Windows 11 22H2, so delegating the phone here would leave
//! every Windows 10 machine with no way to sign in from a phone at all.
//!
//! ## One dialog per method, not one dialog for all three
//!
//! The wallet asks the person to choose — security key, phone, or this device —
//! on a screen it draws itself, and that choice must survive into whatever
//! Windows shows next. So every call carries an [`Attachment`]:
//!
//! * `SecurityKey` → `CROSS_PLATFORM`. Windows Hello stays out of the dialog.
//! * `ThisDevice` → `PLATFORM`. Windows Hello, and nothing else.
//!
//! What the attachment CANNOT do is take the phone out of the `SecurityKey`
//! dialog, because hybrid is itself a cross-platform attachment. At API version
//! 7 there is no transport filter and no credential hints on the
//! make-credential options, so a registration dialog on Windows 11 22H2+ may
//! still offer "use your phone" beside the security key. An assertion for a
//! KNOWN credential is pinned harder — its allow-list entry names the wire —
//! and the discoverable-credential sign-in has no allow list and so no lever.
//! Closing that last gap means not using `webauthn.dll` for the USB port at
//! all; see the CCID note in the feature's `ctap-client-surface.md`.
//!
//! ## What is NOT delegated
//!
//! The clientDataJSON. Windows takes it as a byte buffer and hashes it; it does
//! not build it. So the same `client_data_json()` this wallet uses for CTAP2
//! builds it here too, and the join with the core's parsers — the property that
//! a key minted on any platform derives the same address — survives untouched.
//!
//! ## Pointer discipline
//!
//! Every `WEBAUTHN_*` request struct holds bare pointers into buffers this
//! crate owns. They are held in `Pin<Box<..>>` and kept alive across the call
//! by name, never by luck: a temporary that drops at the end of its statement
//! leaves the API reading freed memory. The shape is borrowed from
//! `webauthn-authenticator-rs`, which learned it the same way everyone does.
//!
//! ## Verification status
//!
//! **Type-checked for Windows, never run on it.** Nobody on this project has a
//! Windows machine yet. Every struct field and signature below is checked by
//! `cargo check --target x86_64-pc-windows-gnu`; none of the behaviour is
//! confirmed. Treat the first run on real hardware as the real review.

#![cfg_attr(not(windows), allow(dead_code))]

/// Which authenticator the Windows dialog is allowed to offer.
///
/// This is `dwAuthenticatorAttachment`, named for what the wallet's three key
/// methods mean rather than for the WebAuthn word. It is the ONE lever a caller
/// has over the picker `webauthn.dll` draws, and this wallet uses it to keep
/// its own three methods from collapsing into one dialog that offers all of
/// them: the person already chose, on a screen this app drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attachment {
    /// A removable security key — `CROSS_PLATFORM`. Windows Hello is excluded.
    ///
    /// It does NOT exclude the phone: hybrid is itself a cross-platform
    /// attachment, and at API version 7 there is no transport filter on the
    /// make-credential options. An assertion for a KNOWN credential is pinned
    /// harder — its allow-list entry carries `WEBAUTHN_CTAP_TRANSPORT_USB`.
    SecurityKey,
    /// Windows Hello — `PLATFORM`. Nothing removable, no phone.
    ///
    /// A Hello credential is sealed to this machine's TPM: it cannot be carried
    /// to the phone, the browser, or another desktop. That is a real limit, not
    /// a defect, and the wallet answers it the same way it answers a single
    /// security key — by asking for a second key before the wallet is finished.
    ThisDevice,
}

/// Everything a make-credential needs, other than the window it hangs from.
///
/// A struct rather than seven positional parameters. Two of them are `&str`
/// names and two are `&str` identities, which at a call site is four strings in
/// a row with nothing but their order keeping them apart — and the order is the
/// kind of thing that stays wrong for a long time, because swapping `rp_name`
/// and `user_name` produces a credential that works and is labelled wrong.
pub struct RegisterRequest<'a> {
    /// The relying party. Part of the wallet's identity: change it and every
    /// existing wallet becomes unreachable.
    pub rp_id: &'a str,
    pub rp_name: &'a str,
    /// `name ‖ NUL ‖ uuid`, from the core's builder.
    pub user_id: &'a [u8],
    pub user_name: &'a str,
    /// Built by this app, not by Windows — see the crate docs.
    pub client_data_json: &'a str,
    /// Credentials this wallet already holds, so the key refuses to mint a
    /// second one on the same authenticator.
    pub exclude_credential_ids: &'a [Vec<u8>],
    /// Which single method the person chose.
    pub attachment: Attachment,
}

/// A completed registration, as bytes.
#[derive(Debug, Clone)]
pub struct Registered {
    pub credential_id: Vec<u8>,
    pub attestation_object: Vec<u8>,
    /// Echoed back rather than rebuilt: what was SIGNED is what must be stored.
    pub client_data_json: String,
    /// `usb`, `nfc`, `ble`, `internal`, `hybrid` — whichever Windows says it
    /// used. Empty when it declines to say.
    pub transport: String,
    /// `platform` when Windows Hello answered, `cross-platform` for a security
    /// key, empty when unknown.
    pub attachment: String,
}

/// A completed assertion, as bytes.
#[derive(Debug, Clone)]
pub struct Asserted {
    pub credential_id: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    /// DER, as the authenticator produced it. Normalisation belongs to the
    /// core, which already does it for every other path.
    pub signature: Vec<u8>,
    /// The user handle, when one came back. Absent is a different fact from
    /// empty, and the core's name resolution branches on it.
    pub user_id: Option<Vec<u8>>,
    pub client_data_json: String,
}

/// What went wrong, in the vocabulary the shell already classifies by.
///
/// Windows answers with an `HRESULT`, and `WebAuthNGetErrorName` turns that
/// into the WebAuthn DOM exception name a browser would have raised —
/// `NotAllowedError`, `InvalidStateError`, `NotSupportedError`. That is a
/// better mapping than guessing at numbers, and it is the same vocabulary the
/// web client's `classify()` already reads.
#[derive(Debug, Clone)]
pub enum WinError {
    /// The API is not present — a Windows older than 10 build 1903.
    Unavailable,
    /// The person declined, or the dialog timed out waiting for them.
    Cancelled,
    /// The chosen authenticator already holds an excluded credential.
    AlreadyRegistered,
    /// No authenticator this request can use.
    NotSupported,
    /// Anything else, with Windows' own words.
    Other(String),
}

impl core::fmt::Display for WinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable => write!(
                f,
                "this version of Windows has no WebAuthn API (Windows 10 build 1903 or later is required)"
            ),
            Self::Cancelled => write!(f, "the request was declined"),
            Self::AlreadyRegistered => write!(
                f,
                "that authenticator already holds one of this wallet's keys"
            ),
            Self::NotSupported => write!(f, "no usable authenticator"),
            Self::Other(message) => write!(f, "{message}"),
        }
    }
}

/// How long Windows waits for the person before giving up: the core's
/// prompt deadline (`passkey_end::SIGN_TIMEOUT_MS`), which is also the clock
/// the core reads a cancel against — a cancel at it is "timed out", before it
/// the person's. The CTAP2 path's exchange budget is the same figure, so the
/// two platforms feel the same and neither is the one that "gives up too
/// fast".
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive number of milliseconds, far below u32::MAX"
)]
pub const TIMEOUT_MS: u32 = vela_core::app::passkey_end::SIGN_TIMEOUT_MS as u32;

const _: () = assert!(TIMEOUT_MS == 120_000, "the core's deadline, whole");

#[cfg(windows)]
mod api;

#[cfg(windows)]
pub use api::{assert, platform_available, register, supported};

#[cfg(windows)]
mod wire;

#[cfg(windows)]
pub use wire::{assertion_from, registration_from};

/// Off Windows this crate compiles to nothing callable, so the desktop app can
/// depend on it unconditionally and let `cfg` decide at the call site.
#[cfg(not(windows))]
pub fn supported() -> bool {
    false
}
