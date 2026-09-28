//! The two ceremonies, over a security key.
//!
//! A browser runs these inside `navigator.credentials`; iOS and Android inside
//! a system passkey service. The desktop has none of those, so it performs them
//! itself — through the core's [`vela_core::ctap::ceremony`] client, which owns
//! the ORDER of the CTAP2 commands and every decision between them. What lives
//! here is only what is this platform's to own: opening the USB device (the
//! [`Cable`]), the prompts and randomness (the [`Host`]), the Windows path
//! (where the OS is the only allowed client), and the mapping of a ceremony
//! failure into the flow's `FailureKind`.
//!
//! **The output shape is the contract.** Everything downstream — public-key
//! extraction, the versioned attestation, the backup-state flag the second-key
//! gate reads, the Safe address — parses these two structs. A desktop-minted
//! key and a browser-minted key must be indistinguishable by the time they
//! leave this file, or the same authenticator derives two different wallets.
//!
//! ## The one judgement call
//!
//! Classifying a failure. It is deliberately narrow (see the operations
//! contract): a cancellation is `cancelled`, a missing or unusable
//! authenticator is `not_supported`, a key that cannot store a discoverable
//! credential is `not_discoverable`, and everything else is `other` carrying
//! the platform's own words — unprettified, because they go into a bug report.

use std::sync::Arc;

use vela_core::app::{Assertion, FailureKind, KeyMethod, Registration};
use vela_core::cable::session::CableInitiator;
use vela_core::ctap::ceremony::{
    self, Cable, CableError, CeremonyError, TouchAnnouncer, TouchKind,
};
use vela_core::primitives;
// The windows path and the client-data tests build the envelope directly.
#[cfg(any(windows, test))]
use vela_core::types::ClientDataKind;

use crate::ctap::cable::{self, HybridError};
#[cfg(not(target_os = "linux"))]
use crate::ctap::ccid;
use crate::ctap::usb::{SecurityKey, TouchNotifier, TouchRequest, UsbError};

/// The relying party every Vela passkey is bound to. A passkey cannot be moved
/// between relying parties, so this string is part of the wallet's identity:
/// change it and every existing wallet becomes unreachable from this app.
pub const RELYING_PARTY: &str = "getvela.app";
const RELYING_PARTY_NAME: &str = "Vela Wallet";

/// The origin the ceremony's clientDataJSON claims. Matches the one the core
/// puts in a software group proof, so a member proof and a group proof read
/// alike; the registry contract never inspects it.
const ORIGIN: &str = "https://getvela.app";

/// What the desktop reports about an authenticator it reached over USB. The
/// browser reports these from the credential; here they are what is true by
/// construction — a removable key on a cable.
const ATTACHMENT_CROSS_PLATFORM: &str = "cross-platform";
// Unreachable on Windows, where `webauthn.dll` reports the transport itself.
#[cfg_attr(windows, allow(dead_code))]
const TRANSPORT_USB: &str = "usb";
/// What a key reached over its smart-card interface reports. Both wires,
/// because a CCID key is the same key that answers over NFC — this is the exact
/// string `ctap_register_ccid` gives the iOS and Android clients, and a
/// credential must read the same however it was minted.
#[cfg(not(target_os = "linux"))]
const TRANSPORT_USB_NFC: &str = "usb,nfc";
/// A key reached over caBLE lives on a phone; it is a hybrid-transport
/// authenticator, and the credential records that so a later flow knows the key
/// is not on this desk.
const TRANSPORT_HYBRID: &str = "hybrid";
/// The label the touch prompt names while the phone shows its own approval
/// sheet. Not localised yet — the QR card carries the localised copy; this is
/// only interpolated into "waiting for {product}" during the brief assertion.
const HYBRID_PRODUCT: &str = "your phone";

/// A ceremony that failed, in the vocabulary the core branches on.
#[derive(Debug)]
pub struct PasskeyFailure {
    pub kind: FailureKind,
    /// The platform's own words. `None` for a classified failure, whose copy
    /// comes from the classification.
    pub message: Option<String>,
}

// Only the Windows mapping and the macOS platform-vault module construct these
// directly; everywhere else the classification arrives from the core ceremony.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
impl PasskeyFailure {
    pub(crate) fn other(message: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Other,
            message: Some(message.into()),
        }
    }

    /// The person closed the sheet. Not an error, and the UI knows the kind.
    pub(crate) fn cancelled() -> Self {
        Self {
            kind: FailureKind::Cancelled,
            message: None,
        }
    }

    pub(crate) fn classified(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            // `not_supported` on desktop is the one classified failure that
            // still carries words: "there is no passkey service here" is not
            // the same sentence as "plug in your key", and the sheet has to be
            // able to say the second one.
            message: Some(message.into()),
        }
    }

    /// The phone's QR was up for the whole scan window and no phone answered
    /// (083 W19). Nobody declined and nothing failed — a dApp request keeps
    /// waiting on it rather than hearing -32603.
    pub(crate) fn scan_ran_out(&self) -> bool {
        self.kind == FailureKind::Other && self.message.as_deref() == Some(cable::NO_ADVERT)
    }

    /// The phone scanned, and the connection to it never came up — its relay
    /// tunnel, or the encrypted channel through it (083 H4). Nothing was asked
    /// of the phone, so nothing was signed, and another try can work: a dApp
    /// request stays open for one rather than hearing -32603.
    pub(crate) fn phone_link_failed(&self) -> bool {
        self.kind == FailureKind::Other
            && self
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with(cable::PHONE_LINK_FAILED))
    }

    /// The phone was connected and asked, and the connection dropped before
    /// its answer came back (083 H4 review): the tunnel went silent, or its
    /// socket failed. Whatever the phone did, nothing signed reached this
    /// desktop — and the desktop sends only what it holds — so another try
    /// can work, and cannot send anything twice. A tunnel the phone closed is
    /// not this: that is its own cancel.
    pub(crate) fn phone_dropped(&self) -> bool {
        self.kind == FailureKind::Other
            && self
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with(cable::PHONE_DROPPED))
    }

    /// Which of the phone's stops this is, when it is one another try can
    /// fix (083): the scan ran out, the connection never came up, or it
    /// dropped once the phone was asked.
    pub(crate) fn phone_stop(&self) -> Option<crate::executor::sign_request::PhoneStop> {
        use crate::executor::sign_request::PhoneStop;
        if self.scan_ran_out() {
            Some(PhoneStop::ScanExpired)
        } else if self.phone_link_failed() {
            Some(PhoneStop::LinkFailed)
        } else if self.phone_dropped() {
            Some(PhoneStop::Dropped)
        } else {
            None
        }
    }
}

/// A core ceremony failure, in this shell's type. Same shape, different enum —
/// the ceremony's vocabulary lives beside the CTAP client so a shell without
/// the crux machines can still use it.
fn ceremony_failure(error: CeremonyError) -> PasskeyFailure {
    // The phone (or the relay) closed the tunnel while we were waiting on it:
    // the person cancelled on the phone, or it had no credential for this
    // relying party. Either way it is the phone's answer, not an error of
    // ours — a cancellation, named as one, rather than "Other" with a
    // transport sentence in it (spec 038 finding 18).
    if error
        .message
        .as_deref()
        .is_some_and(|message| message.contains(cable::TUNNEL_CLOSED))
    {
        return PasskeyFailure::classified(FailureKind::Cancelled, cable::PHONE_ENDED);
    }
    PasskeyFailure {
        kind: match error.kind {
            ceremony::FailureKind::Cancelled => FailureKind::Cancelled,
            ceremony::FailureKind::NotSupported => FailureKind::NotSupported,
            ceremony::FailureKind::NotDiscoverable => FailureKind::NotDiscoverable,
            ceremony::FailureKind::Other => FailureKind::Other,
        },
        message: error.message,
    }
}

/// What can go wrong on this platform's cable, in the ceremony's vocabulary.
// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
fn cable_error(error: UsbError) -> CableError {
    match error {
        UsbError::NoKeyPresent => CableError::NoKeyPresent,
        UsbError::AccessDenied { product, detail } => CableError::AccessDenied { product, detail },
        UsbError::TimedOut => CableError::TimedOut,
        UsbError::Ctap(status) => CableError::Ctap(status),
        other => CableError::Other(other.to_string()),
    }
}

/// A device error, classified all the way to the flow's vocabulary.
// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
fn device_failure(error: UsbError) -> PasskeyFailure {
    ceremony_failure(ceremony::failure_for(cable_error(error)))
}

/// A hybrid-transport setup error, in the flow's vocabulary. Bluetooth being
/// absent is `not_supported` — caBLE cannot run without a radio; everything else
/// (no phone answered, a bad tunnel) is `other` carrying its own words.
fn hybrid_failure(error: HybridError) -> PasskeyFailure {
    match error {
        HybridError::Bluetooth(_) => {
            PasskeyFailure::classified(FailureKind::NotSupported, error.to_string())
        }
        // Dismissing the QR is dismissing the ceremony — what closing the
        // system's passkey sheet already is on every other path.
        HybridError::Cancelled => PasskeyFailure::cancelled(),
        // The person cancelled on the phone while it connected: the phone's
        // answer, as its hang-up mid-prompt is (`ceremony_failure`) — the
        // request goes back to its form, with no card that blames a network
        // (083 H4 review).
        HybridError::PhoneEnded => {
            PasskeyFailure::classified(FailureKind::Cancelled, error.to_string())
        }
        other => PasskeyFailure::other(other.to_string()),
    }
}

/// Asks the person for their security key's PIN, and blocks until they answer.
///
/// `None` means they dismissed the dialog, which is a cancellation and not an
/// error. Blocking is correct here: this runs on a background thread, and the
/// CTAP2 session it belongs to is already holding the device open.
pub type PinRequester = Arc<dyn Fn(PinRequest) -> Option<String> + Send + Sync>;

/// What to tell the person when asking for the PIN.
#[derive(Clone, Debug)]
pub struct PinRequest {
    /// The authenticator's product string, so the dialog can name the key on
    /// the desk rather than "your authenticator".
    pub product: String,
    /// Which key is asking. NOT shown — it is the HID path, and its only job is
    /// to keep one key's PIN from being offered to another. Two keys of the
    /// same model report the same product string, so the product cannot do it.
    pub device: String,
    /// Attempts left before the key locks itself. A locked key can only be
    /// recovered by a RESET, which destroys every credential on it — including
    /// this wallet's founding key — so this number is shown, never hidden.
    pub retries: Option<u32>,
    /// A previous attempt in this same session was refused.
    pub retry: bool,
}

/// One wallet a security key holds for this relying party.
#[derive(Clone, Debug)]
pub struct CredentialChoice {
    /// The wallet's name, read out of the user handle the credential was minted
    /// with (`name‖NUL‖uuid`). Empty when the authenticator volunteered no
    /// handle, which a very old credential may not.
    pub name: String,
    /// Hex. Only shown when two rows share a name — otherwise it is noise the
    /// person has no use for.
    pub credential_id: String,
    /// The key these all live on, for the row's second line.
    pub product: String,
}

/// Asks which of several wallets on one key to sign in as, and blocks.
///
/// `None` is a dismissal, which is a cancellation. Only called when the key
/// reports more than one: a single credential is not a choice.
pub type CredentialPicker = Arc<dyn Fn(Vec<CredentialChoice>) -> Option<usize> + Send + Sync>;

/// Shows (or clears) the caBLE QR the person scans with their phone. `Some` with
/// the `FIDO:/…` payload while the hybrid handshake waits for a scan; `None`
/// once the phone's advert is found (083 H5) or the attempt ends.
pub type QrNotifier = Arc<dyn Fn(Option<String>) + Send + Sync>;

pub use crate::ctap::usb::CancelProbe;

/// Everything a ceremony needs from the screen that started it.
#[derive(Clone)]
pub struct Ceremony {
    /// Called with what the key is waiting for when it starts waiting, and
    /// with `None` when it stops.
    pub touch: TouchNotifier,
    pub pin: PinRequester,
    pub pick: CredentialPicker,
    /// Shows the caBLE QR while a hybrid ceremony waits for the phone.
    pub qr: QrNotifier,
    /// Has the person dismissed that QR? Polled by the scan, which is the one
    /// open-ended wait a hybrid ceremony has.
    pub cancelled: CancelProbe,
    /// Spec 075: the Trusted Signer's own channel to the screen — where the
    /// page is, the page to open, the pairing code to confirm, and how the
    /// last attempt ended. A ceremony with `method = trusted_signer` runs
    /// there instead of on any authenticator this machine can reach.
    pub trusted_signer: Arc<crate::executor::trusted_signer::Channel>,
    /// The app window the Windows dialog parents itself to.
    ///
    /// Read on exactly one platform, because it is the only one where the
    /// passkey dialog belongs to the OS. Everywhere else this shell draws its
    /// own prompts and there is nothing to parent.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub window: WindowHandle,
}

// ---------------------------------------------------------------------------
// The core ceremony's two seams, on this platform
// ---------------------------------------------------------------------------

/// The USB HID cable, as the core ceremony sees it.
// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
struct UsbCable<'a> {
    key: SecurityKey,
    touch: &'a TouchNotifier,
}

impl Cable for UsbCable<'_> {
    fn exchange(
        &mut self,
        request: &[u8],
        touch: Option<TouchKind>,
    ) -> Result<Vec<u8>, CableError> {
        self.key
            .cbor(request, touch.map(|kind| (self.touch, kind)))
            .map_err(cable_error)
    }

    fn cancel(&mut self) {
        self.key.cancel();
    }

    fn product(&self) -> &str {
        self.key.product()
    }

    fn path(&self) -> &str {
        self.key.path()
    }
}

/// The person and the platform, as the core ceremony sees them.
struct DesktopHost<'a> {
    ceremony: &'a Ceremony,
}

impl ceremony::Host for DesktopHost<'_> {
    fn pin(&self, request: ceremony::PinRequest) -> Option<String> {
        (self.ceremony.pin)(PinRequest {
            product: request.product,
            device: request.device,
            retries: request.retries,
            retry: request.retry,
        })
    }

    fn pick(&self, choices: Vec<ceremony::CredentialChoice>) -> Option<usize> {
        (self.ceremony.pick)(
            choices
                .into_iter()
                .map(|choice| CredentialChoice {
                    name: choice.name,
                    credential_id: choice.credential_id,
                    product: choice.product,
                })
                .collect(),
        )
    }

    fn random(&self, len: usize) -> Vec<u8> {
        random(len)
    }

    fn note(&self, line: &str) {
        eprintln!("[vela-wallet] {line}");
    }
}

// ---------------------------------------------------------------------------
// Windows: the OS runs the ceremony
// ---------------------------------------------------------------------------
//
// The USB half of this file talks to a security key over a cable. On Windows it
// cannot: since Windows 10 build 1903 a non-elevated process may not open a
// FIDO HID device, because the OS reserves them for `webauthn.dll`. So for the
// key in the port, the platform's CTAP client is the only route — and it draws
// its own picker, touch prompt and PIN prompt, which is why none of this
// shell's three hardware dialogs appear there.
//
// **The scope of that is the USB port and nothing else.** The caBLE half below
// runs on Windows exactly as it does on the other two desktops: a BLE
// advertisement scan and a WebSocket, neither of which the lockdown touches.
// Delegating the phone to Windows as well would have been a real loss —
// `webauthn.dll` only grew its own QR flow in Windows 11 22H2, so every Windows
// 10 machine would have had no way to sign in from a phone at all.
//
// And the ceremony that IS delegated asks for a cross-platform authenticator,
// so Windows Hello stays out of the dialog. A Hello credential is sealed to one
// machine's TPM; a wallet founded on one cannot be reached from the phone, the
// browser, or the other two desktops. The cross-device answer here is the QR.
//
// The clientDataJSON is still OURS. Windows takes it as bytes and hashes it; it
// does not build it. So the join with the core's parsers is identical on all
// three desktops.

/// The app window the OS dialog parents itself to.
///
/// Windows needs one to know where to put its sheet, and gpui gives us a real
/// one — `webauthn-authenticator-rs` has to create a 1×1 window for this
/// because it is a library called from console apps that have none.
pub type WindowHandle = isize;

#[cfg(windows)]
fn win_failure(error: vela_passkey_win::WinError) -> PasskeyFailure {
    use vela_passkey_win::WinError;
    match error {
        WinError::Cancelled => PasskeyFailure {
            kind: FailureKind::Cancelled,
            message: None,
        },
        WinError::Unavailable | WinError::NotSupported => PasskeyFailure::classified(
            FailureKind::NotSupported,
            "This version of Windows cannot run a passkey ceremony. Windows 10 build 1903 or later is required.",
        ),
        other => PasskeyFailure::other(other.to_string()),
    }
}

/// The person's chosen method, as the one lever Windows gives us over its own
/// picker.
///
/// `Hybrid` never reaches here — `register` and `assert` hand it to the app's
/// own caBLE client before the Windows half is consulted. Neither does
/// `TrustedSigner` (spec 075): `executor::perform` answers it before any
/// ceremony starts. Both map to the security key only so the match is total,
/// which is also what the non-Windows path does with a method it did not
/// route away — there, anything left over runs the one USB ceremony.
#[cfg(windows)]
fn win_attachment(method: KeyMethod) -> vela_passkey_win::Attachment {
    match method {
        KeyMethod::Platform => vela_passkey_win::Attachment::ThisDevice,
        KeyMethod::SecurityKey | KeyMethod::Hybrid | KeyMethod::TrustedSigner => {
            vela_passkey_win::Attachment::SecurityKey
        }
    }
}

#[cfg(windows)]
fn register_windows(
    window: WindowHandle,
    name: &str,
    exclude_credential_ids: &[String],
    method: KeyMethod,
) -> Result<Registration, PasskeyFailure> {
    let client_data = ceremony::client_data_json(ClientDataKind::Create, &random(32), ORIGIN);
    let exclude: Vec<Vec<u8>> = exclude_credential_ids
        .iter()
        .filter_map(|id| primitives::from_hex(id).ok())
        .collect();

    vela_passkey_win::register(
        window,
        &vela_passkey_win::RegisterRequest {
            rp_id: RELYING_PARTY,
            rp_name: RELYING_PARTY_NAME,
            user_id: &user_handle(name),
            user_name: name,
            client_data_json: &client_data,
            exclude_credential_ids: &exclude,
            attachment: win_attachment(method),
        },
    )
    .map(vela_passkey_win::registration_from)
    .map_err(win_failure)
}

#[cfg(windows)]
fn assert_windows(
    window: WindowHandle,
    challenge: &[u8],
    credential_id: Option<&str>,
    method: KeyMethod,
) -> Result<Assertion, PasskeyFailure> {
    let client_data = ceremony::client_data_json(ClientDataKind::Get, challenge, ORIGIN);
    let pinned = credential_id.and_then(|id| primitives::from_hex(id).ok());

    vela_passkey_win::assert(
        window,
        RELYING_PARTY,
        &client_data,
        pinned.as_deref(),
        win_attachment(method),
    )
    .map(vela_passkey_win::assertion_from)
    .map_err(win_failure)
}

/// Is there a system passkey service on this desktop — a real "This device"?
///
/// `false` on macOS and Linux: gpui reaches no platform authenticator on either
/// (the AS API on macOS is future work), which is why the method row there is
/// greyed with a sentence saying to plug a key in. `true` on Windows when
/// Windows Hello is not merely present but ENROLLED — a machine with no TPM, or
/// where nobody has set up a PIN, face or fingerprint, answers `false`, because
/// a row that fails when tapped is worse than a row that says why.
///
/// **Cached for the life of the process.** The answer costs a `webauthn.dll`
/// round trip that asks the TPM, and the method picker asks on every frame.
/// Somebody who enrols a fingerprint while the wallet is open sees the row
/// unlock on the next launch, which is the right trade for a per-frame call.
pub fn platform_supported() -> bool {
    // The parallel space's fixture reports `platform` attachment, so the row
    // it answers as must be offered. Not cached: the space is a process-wide
    // switch, and reading one env var per frame is cheaper than a stale row.
    #[cfg(feature = "dev-fixtures")]
    if crate::parallel_space::active() {
        return true;
    }
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        #[cfg(windows)]
        {
            vela_passkey_win::platform_available()
        }
        #[cfg(target_os = "macos")]
        {
            // The AS platform-passkey API (macOS 12+). Whether the SIGNATURE
            // satisfies the getvela.app association is not probed — the
            // ceremony's own error names the fix better than a greyed row.
            super::platform_macos::available()
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            false
        }
    })
}

/// Is a passkey ceremony possible on this machine at all?
///
/// This asks whether the HID subsystem is REACHABLE, not whether a key is
/// plugged in right now. The distinction is the whole recovery path: answering
/// `false` for an empty USB port would make the core raise "this device cannot
/// create a wallet", which is untrue and has no way back. A missing key is
/// reported later, by the ceremony, as `not_supported` with a message naming
/// it — and plugging one in and pressing 重试 then works.
pub fn supported() -> bool {
    // In the parallel space a ceremony needs no subsystem at all.
    #[cfg(feature = "dev-fixtures")]
    if crate::parallel_space::active() {
        return true;
    }
    #[cfg(windows)]
    {
        // Not the HID subsystem: on Windows that answer is always "no" for a
        // non-elevated process, and it would be the wrong question anyway.
        //
        // This answers for the USB ceremony only. The caBLE one needs no
        // `webauthn.dll` and is not asked about here — probing for a Bluetooth
        // adapter means standing a tokio runtime up on the support check, which
        // is a lot of machinery for a case the answer barely moves: version 1
        // of this API is build 1903, and every Windows that ships without it is
        // out of support (1809 LTSC excepted, where a phone would in fact still
        // work and this will still say no).
        vela_passkey_win::supported()
    }
    #[cfg(not(windows))]
    {
        // Asked on THE HID thread (`ctap::usb::hid_thread`), and cached for the
        // life of the process. The thread is the load-bearing half on macOS:
        // `HidApi::new` schedules a process-global IOHIDManager on the CURRENT
        // thread's run loop. Called from gpui's libdispatch workers it trapped
        // in `CFRunLoopAddSource`; moved to a one-shot `std::thread` it stopped
        // trapping HERE and started trapping later — that thread initialised
        // the manager and then ended, leaving every later enumeration a dead
        // run loop (the USB sign-in crash of 2026-09-19). One immortal thread
        // for every hidapi entry is the fix for both. The cache stays because
        // "is the HID subsystem reachable" does not change mid-session — a key
        // plugged in later is the ceremony's business, not this gate's.
        static REACHABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *REACHABLE.get_or_init(crate::ctap::usb::hid_reachable)
    }
}

/// `RegisterPasskey` — mint a founding key.
///
/// Which ceremony runs is decided HERE, once, and the decision is about the
/// WIRE rather than the operating system. A removable key has TWO wires — HID
/// and CCID — and which is tried first is the one genuinely per-platform part:
/// macOS leads with HID and falls back to CCID when no key answers; Windows can
/// only do CCID, and falls back to `webauthn.dll`; Linux has HID alone.
///
///   * [`KeyMethod::Hybrid`] — the phone that scans the QR, over caBLE. All
///     three desktops run this themselves, Windows included; nothing in it
///     touches a FIDO HID device, so the 1903 lockdown has no say in it.
///   * [`KeyMethod::Platform`] — "this device". Only Windows has one reachable
///     from gpui (Windows Hello, through `webauthn.dll`); the method picker
///     greys the row out everywhere else, so it does not arrive there.
///   * [`KeyMethod::SecurityKey`] — the key in the USB port. macOS and Linux
///     drive CTAP2 over HID themselves; Windows may not, and hands that one
///     ceremony to `webauthn.dll`.
///
/// Every caller sees one function.
pub fn register(
    name: &str,
    exclude_credential_ids: &[String],
    method: KeyMethod,
    ceremony: &Ceremony,
) -> Result<Registration, PasskeyFailure> {
    // The parallel space (spec 032 US0): the fixed keyset mints the key, no
    // cable, whatever method the person picked — the method still labels the
    // key row, so a fixture wallet reads like a real one. Both gates are
    // `parallel_space::active`'s; without the feature this line is dead.
    #[cfg(feature = "dev-fixtures")]
    if crate::parallel_space::active() {
        return crate::parallel_space::signer::register(exclude_credential_ids);
    }
    // The scan method mints the key on a phone over caBLE. The choice labels the
    // key row either way; here it only decides which transport carries the
    // make-credential.
    if method == KeyMethod::Hybrid {
        return register_hybrid(name, exclude_credential_ids, ceremony);
    }
    // "This device" on macOS: the AS platform vault (Touch ID / iCloud
    // Keychain). The exclude list is deliberately not forwarded yet —
    // `excludedCredentials` exists only from macOS 14, and the platform vault
    // cannot hold one of OUR keys the person is excluding unless it minted it,
    // which this same path would have refused; revisit when a second platform
    // key per wallet is a real flow.
    #[cfg(target_os = "macos")]
    if method == KeyMethod::Platform {
        return super::platform_macos::register(name, user_handle(name), random(32));
    }
    #[cfg(windows)]
    {
        // A removable key, on Windows: the smart-card wire FIRST. It is the one
        // route here where this app is its own CTAP client for a key on the
        // desk — its own picker, PIN and touch prompts, no system sheet, and no
        // chance of Windows offering Hello or a phone in a dialog nobody asked
        // for. `webauthn.dll` catches everything that does not answer: a key
        // older than YubiKey firmware 5.8, one with CCID switched off, or a
        // machine whose Smart Card service is stopped.
        if method == KeyMethod::SecurityKey
            && let Some(result) = register_ccid(name, exclude_credential_ids, ceremony)
        {
            return result;
        }
        register_windows(ceremony.window, name, exclude_credential_ids, method)
    }
    #[cfg(not(windows))]
    {
        register_ctap(name, exclude_credential_ids, ceremony)
    }
}

/// `SignProof` / `SignMemberProof` / `AuthenticatePasskey` — one assertion.
///
/// `credential_id` is `None` for the "who are you?" ceremony a sign-in starts
/// with: an empty allow list is what asks for any discoverable credential.
///
/// The hybrid branch routes on the method the person chose, not on whether a
/// credential id is known: a scan-method sign-in reaches the phone with no allow
/// list, and a scan-method proof (recovery's second signature) reaches the SAME
/// phone credential through an allow list pinned to it. The USB and Windows paths
/// keep a known id on the key in the port, as before.
pub fn assert(
    challenge: &[u8],
    credential_id: Option<&str>,
    method: KeyMethod,
    ceremony: &Ceremony,
) -> Result<Assertion, PasskeyFailure> {
    // The parallel space signs with the fixed keyset — see `register`.
    #[cfg(feature = "dev-fixtures")]
    if crate::parallel_space::active() {
        return crate::parallel_space::signer::assert(challenge, credential_id);
    }
    // The scan method signs over caBLE, whether the phone offers what it holds
    // (sign-in, no credential id) or is pinned to one credential (recovery's
    // second signature, credential id known — the allow list keeps a multi-key
    // phone from answering with a different passkey than the first).
    if method == KeyMethod::Hybrid {
        return assert_hybrid(challenge, credential_id, ceremony);
    }
    // "This device" on macOS: the platform vault signs, pinned or discoverable
    // exactly like the caBLE branch above. A proof whose sign-in ran here comes
    // back here — the method travels with the operation for that reason.
    #[cfg(target_os = "macos")]
    if method == KeyMethod::Platform {
        return super::platform_macos::assert(challenge, credential_id);
    }
    #[cfg(windows)]
    {
        // See `register`: the smart-card wire before the system dialog.
        if method == KeyMethod::SecurityKey
            && let Some(result) = assert_ccid(challenge, credential_id, ceremony)
        {
            return result;
        }
        assert_windows(ceremony.window, challenge, credential_id, method)
    }
    #[cfg(not(windows))]
    {
        assert_ctap(challenge, credential_id, ceremony)
    }
}

#[cfg(not(windows))]
fn register_ctap(
    name: &str,
    exclude_credential_ids: &[String],
    ceremony: &Ceremony,
) -> Result<Registration, PasskeyFailure> {
    // Hybrid never arrives here — `register` took it. Every method that does is
    // the one USB ceremony (the picker presents platform as
    // unavailable-with-a-reason).
    let key = match open(ceremony) {
        Ok(key) => key,
        // Nothing answered on HID. Before saying so, try the key's OTHER USB
        // wire: a YubiKey with its HID interface switched off in `ykman` still
        // answers on CCID, and so does a card in a PC/SC reader. Only
        // `NoKeyPresent` falls through — a key that answered and then failed has
        // already had its ceremony, and re-running it on another wire would ask
        // a person the same question twice.
        Err(UsbError::NoKeyPresent) => {
            #[cfg(not(target_os = "linux"))]
            if let Some(result) = register_ccid(name, exclude_credential_ids, ceremony) {
                return result;
            }
            return Err(device_failure(UsbError::NoKeyPresent));
        }
        Err(error) => return Err(device_failure(error)),
    };
    let mut cable = UsbCable {
        key,
        touch: &ceremony.touch,
    };
    let host = DesktopHost { ceremony };
    let registration = ceremony::Client {
        cable: &mut cable,
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(name, exclude_credential_ids)
    .map_err(ceremony_failure)?;

    Ok(Registration {
        credential_id: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        transports: TRANSPORT_USB.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    })
}

#[cfg(not(windows))]
fn assert_ctap(
    challenge: &[u8],
    credential_id: Option<&str>,
    ceremony: &Ceremony,
) -> Result<Assertion, PasskeyFailure> {
    // The caBLE sign-in never arrives here — `assert` took it.
    let opened = match credential_id {
        Some(id) => open_for(id, ceremony),
        None => open(ceremony),
    };
    // See `register_ctap` for why only `NoKeyPresent` reaches the other wire.
    let key = match opened {
        Ok(key) => key,
        Err(UsbError::NoKeyPresent) => {
            #[cfg(not(target_os = "linux"))]
            if let Some(result) = assert_ccid(challenge, credential_id, ceremony) {
                return result;
            }
            return Err(device_failure(UsbError::NoKeyPresent));
        }
        Err(error) => return Err(device_failure(error)),
    };
    let mut cable = UsbCable {
        key,
        touch: &ceremony.touch,
    };
    let host = DesktopHost { ceremony };
    let assertion = ceremony::Client {
        cable: &mut cable,
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(challenge, credential_id)
    .map_err(ceremony_failure)?;

    Ok(Assertion {
        credential_id: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    })
}

// ---------------------------------------------------------------------------
// CCID: the same removable key, on its smart-card interface
// ---------------------------------------------------------------------------
//
// `Option<Result<..>>` rather than `Result<..>`, and the distinction is the
// whole contract: `None` means NO CARD WAS EVER REACHED — nothing was asked of
// anybody, so a caller is free to try another wire. `Some` means a card
// answered and the ceremony ran; its outcome is final, because falling back
// after a person has touched a key (or declined) would ask them the same
// question a second time in a different dialog.

/// The touch prompt for a key on this desk. `remote: false` — the prompt says
/// "touch your security key", which is the caBLE path's one difference.
#[cfg(not(target_os = "linux"))]
fn desk_touch_announcer(ceremony: &Ceremony) -> TouchAnnouncer {
    let touch = Arc::clone(&ceremony.touch);
    Box::new(move |kind, product| {
        touch(Some(TouchRequest {
            kind,
            product: product.to_owned(),
            remote: false,
            // The CCID keepalive poll lives in the core and has no dismissal
            // to watch; no button until it does.
            cancellable: false,
        }));
    })
}

/// Open the smart-card wire, or say why not — one line to stderr, because the
/// caller's only response to a failure here is to try something else, and the
/// reason it could not (no reader / no FIDO applet / no service) is worth having
/// when somebody reports "it used the Windows dialog again".
#[cfg(not(target_os = "linux"))]
fn open_ccid(ceremony: &Ceremony) -> Option<ccid::ApduCableOnCard> {
    match ccid::open_cable() {
        Ok(mut cable) => {
            cable.on_touch(desk_touch_announcer(ceremony));
            Some(cable)
        }
        Err(reason) => {
            eprintln!("[vela-wallet] no security key on the smart-card wire: {reason}");
            None
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn register_ccid(
    name: &str,
    exclude_credential_ids: &[String],
    ceremony: &Ceremony,
) -> Option<Result<Registration, PasskeyFailure>> {
    let mut cable = open_ccid(ceremony)?;
    let host = DesktopHost { ceremony };
    let registration = ceremony::Client {
        cable: &mut cable,
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(name, exclude_credential_ids)
    .map_err(ceremony_failure);
    (ceremony.touch)(None);

    Some(registration.map(|registration| Registration {
        credential_id: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        transports: TRANSPORT_USB_NFC.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    }))
}

#[cfg(not(target_os = "linux"))]
fn assert_ccid(
    challenge: &[u8],
    credential_id: Option<&str>,
    ceremony: &Ceremony,
) -> Option<Result<Assertion, PasskeyFailure>> {
    let mut cable = open_ccid(ceremony)?;
    let host = DesktopHost { ceremony };
    let assertion = ceremony::Client {
        cable: &mut cable,
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(challenge, credential_id)
    .map_err(ceremony_failure);
    (ceremony.touch)(None);

    Some(assertion.map(|assertion| Assertion {
        credential_id: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    }))
}

// ---------------------------------------------------------------------------
// caBLE: the ceremony runs over a phone reached by scanning the QR
// ---------------------------------------------------------------------------

/// Fresh secrets, the QR on screen, and the Noise handshake over the tunnel the
/// scanned phone opens — returning the [`Cable`] a ceremony drives. The QR is
/// shown before the scan (scanning it is what makes the phone advertise) and
/// cleared once the phone has scanned it, or however it ends.
fn run_hybrid(ceremony: &Ceremony, for_get: bool) -> Result<Box<dyn Cable>, PasskeyFailure> {
    let static_seed = random(32);
    let qr_secret = random(16);
    let session = CableInitiator::new(&static_seed, &qr_secret)
        .ok_or_else(|| PasskeyFailure::other("could not start a caBLE session"))?;

    // The QR is exactly Chrome's shape — no channel offer. GMS's caBLE-v2.1
    // parser hard-rejects a QR whose key 6 is anything but its legacy bool
    // (device-found 2026-08-28: GMS phones never connected, iPhones did), and
    // our own authenticators never needed the offer: the channel is chosen by
    // the ADVERT — a PSM in its suffix means BLE, none means the tunnel — and
    // `establish_hybrid` only takes the L2CAP branch where this desktop can
    // connect it (`cable::BLE_CHANNEL_SUPPORTED` gates the branch, not the QR).
    (ceremony.qr)(Some(session.qr_payload(unix_seconds(), for_get)));

    let ephemeral_seed = random(32);
    let touch_notify = Arc::clone(&ceremony.touch);
    let on_touch: TouchAnnouncer =
        Box::new(move |kind, product| touch_notify(Some(phone_prompt(kind, product))));

    let result = cable::establish_hybrid(
        &session,
        HYBRID_PRODUCT.to_owned(),
        &ephemeral_seed,
        Some(on_touch),
        &|| phone_found(ceremony),
        &*ceremony.cancelled,
    );
    hybrid_ready(ceremony, result)
}

/// The phone's card. Over caBLE the "authenticator" is the person's phone; the
/// prompt must say so, not "touch your security key". No Cancel: the phone is
/// mid-ceremony over the tunnel, and nothing here polls.
fn phone_prompt(kind: TouchKind, product: &str) -> TouchRequest {
    TouchRequest {
        kind,
        product: product.to_owned(),
        remote: true,
        cancellable: false,
    }
}

/// The phone's card while its connection comes up (083 H5): the same words,
/// and a Cancel. Unlike the prompt's, this one stops something — the phone
/// has been asked nothing yet, and a dismissal here means it never is
/// ([`hybrid_ready`], [`asked_nothing_yet`]). Without it the card covered
/// the window through the tunnel and the handshake with nothing to press
/// (083 H4 review), where the QR's Cancel had been.
fn phone_connecting() -> TouchRequest {
    TouchRequest {
        cancellable: true,
        ..phone_prompt(TouchKind::Presence, HYBRID_PRODUCT)
    }
}

/// The phone scanned the QR and its advert decrypted (083 H5). The tunnel and
/// the handshake can take seconds more, and the QR stood over them as if
/// nobody had scanned — so it comes down now, and the phone's card says to
/// look at the phone. Not over a QR dismissed in the same instant: the flow
/// the person just cancelled gets no card.
fn phone_found(ceremony: &Ceremony) {
    (ceremony.qr)(None);
    if (ceremony.cancelled)() {
        return;
    }
    (ceremony.touch)(Some(phone_connecting()));
}

/// The hybrid setup, back. The QR comes down however it ended. A dismissal
/// made while the tunnel came up — the connecting card's Cancel, the QR's,
/// a column's close — is honoured here rather than carried on into a prompt
/// for a ceremony the person has walked away from; a phone that did connect
/// is told goodbye, so it is not left waiting for a request. The phone's
/// card goes with a failure or a dismissal (083 H5), and stays for a phone
/// that is connected: its prompt is next.
fn hybrid_ready(
    ceremony: &Ceremony,
    result: Result<Box<dyn Cable>, HybridError>,
) -> Result<Box<dyn Cable>, PasskeyFailure> {
    (ceremony.qr)(None);
    if (ceremony.cancelled)() {
        (ceremony.touch)(None);
        if let Ok(mut cable) = result {
            cable.cancel();
        }
        return Err(PasskeyFailure::cancelled());
    }
    result.map_err(|error| {
        (ceremony.touch)(None);
        hybrid_failure(error)
    })
}

/// The connecting card can still be dismissed in the breath between the
/// handshake and the ceremony's first request (083 H4 review): the phone is
/// then asked nothing, and told goodbye.
fn asked_nothing_yet(cable: &mut dyn Cable, ceremony: &Ceremony) -> Result<(), PasskeyFailure> {
    if (ceremony.cancelled)() {
        (ceremony.touch)(None);
        cable.cancel();
        return Err(PasskeyFailure::cancelled());
    }
    Ok(())
}

/// Wall-clock seconds since the epoch, for the QR's freshness field. The core is
/// clockless; the shell owns the clock.
fn unix_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
        .unwrap_or(0)
}

fn register_hybrid(
    name: &str,
    exclude_credential_ids: &[String],
    ceremony: &Ceremony,
) -> Result<Registration, PasskeyFailure> {
    let mut cable = run_hybrid(ceremony, false)?;
    asked_nothing_yet(cable.as_mut(), ceremony)?;
    let host = DesktopHost { ceremony };
    let outcome = ceremony::Client {
        cable: cable.as_mut(),
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .register(name, exclude_credential_ids);
    // See `assert_hybrid`: the card comes down on every exit, the goodbye only
    // on a success.
    (ceremony.touch)(None);
    let registration = outcome.map_err(ceremony_failure)?;
    cable.cancel();

    Ok(Registration {
        credential_id: registration.credential_id_hex,
        attestation_object_hex: registration.attestation_object_hex,
        client_data_json_hex: registration.client_data_json_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        transports: TRANSPORT_HYBRID.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    })
}

/// The assertion itself, over an already-established cable — split from the
/// transport so the teardown rule below is a unit test rather than a phone.
///
/// `credential_id` is `None` for a sign-in (any discoverable credential) and
/// `Some` for recovery's second signature, where the allow list pins the same
/// credential the first signature used.
fn assert_over(
    cable: &mut dyn Cable,
    ceremony: &Ceremony,
    challenge: &[u8],
    credential_id: Option<&str>,
) -> Result<ceremony::Assertion, PasskeyFailure> {
    asked_nothing_yet(cable, ceremony)?;
    let host = DesktopHost { ceremony };
    let outcome = ceremony::Client {
        cable,
        host: &host,
        rp_id: RELYING_PARTY,
        rp_name: RELYING_PARTY_NAME,
        origin: ORIGIN,
    }
    .assert(challenge, credential_id);
    // The "check your phone" card comes down on EVERY exit. It used to come
    // down only after a success, so a phone that hung up mid-assertion left
    // the card over the failure sheet with no way out (spec 038 finding 18).
    (ceremony.touch)(None);
    let assertion = outcome.map_err(ceremony_failure)?;
    // Say goodbye before dropping the channel: the caBLE shutdown frame lets the
    // phone end its session loop cleanly instead of decrypting the transport
    // teardown as a garbled frame (a BAD_DECRYPT in its log after every success).
    // Only after a success — on a failure the tunnel is usually the thing that
    // died, and the port refuses the write anyway.
    cable.cancel();
    Ok(assertion)
}

fn assert_hybrid(
    challenge: &[u8],
    credential_id: Option<&str>,
    ceremony: &Ceremony,
) -> Result<Assertion, PasskeyFailure> {
    let mut cable = run_hybrid(ceremony, true)?;
    let assertion = assert_over(cable.as_mut(), ceremony, challenge, credential_id)?;
    // The user handle is where a recovered wallet's NAME survives; when a
    // recovery lands on the "Wallet" fallback, this line says which link broke
    // (handle absent vs. handle present but not `name‖NUL‖uuid`).
    eprintln!(
        "[vela-cable] assertion user handle: {}",
        match assertion.user_id_hex.as_deref() {
            None => "absent".to_owned(),
            Some(hex) => format!("{} bytes", hex.len() / 2),
        }
    );

    Ok(Assertion {
        credential_id: assertion.credential_id_hex,
        signature_der_hex: assertion.signature_der_hex,
        authenticator_data_hex: assertion.authenticator_data_hex,
        client_data_json_hex: assertion.client_data_json_hex,
        user_id_hex: assertion.user_id_hex,
        authenticator_attachment: ATTACHMENT_CROSS_PLATFORM.to_owned(),
        // Spec 075: a native ceremony's key does not live behind a page.
        signer_origin: None,
    })
}

// ---------------------------------------------------------------------------
// Opening a device (the transport's business, so it stays here)
// ---------------------------------------------------------------------------

// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
fn nonces() -> impl Fn() -> [u8; 8] {
    || match random(8).try_into() {
        Ok(nonce) => nonce,
        Err(_) => unreachable!("random(8) returns 8 bytes"),
    }
}

/// The key to run a ceremony on when ANY key will do — a registration, or the
/// "who are you?" sign-in. With several plugged in, the one touched wins.
// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
fn open(ceremony: &Ceremony) -> Result<SecurityKey, UsbError> {
    SecurityKey::open_touched(
        &nonces(),
        Some(&ceremony.touch),
        Some(Arc::clone(&ceremony.cancelled)),
    )
}

/// The key that already holds this credential.
///
/// Used whenever the credential id is known — every proof, every membership
/// confirmation, every re-signature. Those are the ceremonies where making all
/// three plugged-in keys blink is asking a person to answer a question the
/// client can answer itself, silently, in a few milliseconds.
///
/// The probe hash is fresh random bytes, never the ceremony's real client-data
/// hash: a probe is discarded, and an assertion signed over the real hash by a
/// key that then loses the race would be a signature nobody asked for.
// Unreachable on Windows, where `webauthn.dll` runs the USB ceremony instead.
#[cfg_attr(windows, allow(dead_code))]
fn open_for(credential_id: &str, ceremony: &Ceremony) -> Result<SecurityKey, UsbError> {
    let Ok(id) = primitives::from_hex(credential_id) else {
        return open(ceremony);
    };
    SecurityKey::open_holding(
        &id,
        RELYING_PARTY,
        &random(32),
        &nonces(),
        Some(&ceremony.touch),
        Some(Arc::clone(&ceremony.cancelled)),
    )
}

/// `name ‖ NUL ‖ uuid`, from the core's builder and this shell's randomness.
#[cfg(any(windows, target_os = "macos", test))]
fn user_handle(name: &str) -> Vec<u8> {
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&random(16));
    ceremony::user_handle(name, uuid)
}

/// All randomness in the flow lives in the shell. This is that shell.
pub fn random(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    if getrandom::fill(&mut bytes).is_err() {
        // There is no safe degraded mode for a wallet whose CSPRNG is gone:
        // every value this function produces is either a challenge or a key.
        unreachable!("the platform CSPRNG is unavailable");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::ctap::Status;
    use vela_core::registry_proof::build_member_proof;
    use vela_core::webauthn;

    /// 083 (review): only the scan window running out is "no phone answered",
    /// which the dApp column shows as a 90-second timeout with Retry. A
    /// Bluetooth stack that stops reporting early says Bluetooth — shown as
    /// that timeout, the real failure was hidden behind a Retry that could
    /// not work.
    #[test]
    fn only_the_window_running_out_is_a_scan_timeout() {
        assert!(hybrid_failure(HybridError::NoAdvert).scan_ran_out());
        let stopped = hybrid_failure(HybridError::Bluetooth(cable::SCAN_STREAM_ENDED.to_owned()));
        assert!(!stopped.scan_ran_out());
        assert_eq!(stopped.kind, FailureKind::NotSupported);
        assert!(!hybrid_failure(HybridError::Cancelled).scan_ran_out());
    }

    /// 083 H4: a phone found whose connection never came up — the tunnel, or
    /// the handshake through it — is the one hybrid SETUP failure another try
    /// can fix. Nothing else is: no phone at all is the scan timeout, no
    /// radio is Bluetooth, a dismissal is the person's — and so (review) is a
    /// phone that hung up while it connected: its own cancel, back to the
    /// form, never "check your network".
    #[test]
    fn only_a_connection_that_never_came_up_is_worth_another_try() {
        use crate::executor::sign_request::PhoneStop;
        for error in [
            HybridError::Tunnel("cannot reach cable.ua5v.com: refused".to_owned()),
            HybridError::Handshake("the phone stopped answering".to_owned()),
        ] {
            let failure = hybrid_failure(error);
            assert!(failure.phone_link_failed(), "{:?}", failure.message);
            assert!(!failure.scan_ran_out());
            assert!(!failure.phone_dropped());
            assert_eq!(failure.phone_stop(), Some(PhoneStop::LinkFailed));
        }
        assert_eq!(
            hybrid_failure(HybridError::NoAdvert).phone_stop(),
            Some(PhoneStop::ScanExpired)
        );
        for error in [
            HybridError::NoAdvert,
            HybridError::Bluetooth("no Bluetooth adapter".to_owned()),
            HybridError::BadAdvert,
            HybridError::PhoneEnded,
            HybridError::Cancelled,
        ] {
            assert!(!hybrid_failure(error).phone_link_failed());
        }
        for error in [
            HybridError::Bluetooth("no Bluetooth adapter".to_owned()),
            HybridError::BadAdvert,
            HybridError::PhoneEnded,
            HybridError::Cancelled,
        ] {
            assert_eq!(hybrid_failure(error).phone_stop(), None);
        }
        let hung_up = hybrid_failure(HybridError::PhoneEnded);
        assert_eq!(hung_up.kind, FailureKind::Cancelled, "the phone's answer");
        assert_eq!(hung_up.message.as_deref(), Some(cable::PHONE_ENDED));
    }

    /// A connected phone whose every exchange fails with `error`.
    struct Failing(fn() -> CableError);
    impl Cable for Failing {
        fn exchange(
            &mut self,
            _request: &[u8],
            _touch: Option<TouchKind>,
        ) -> Result<Vec<u8>, CableError> {
            Err((self.0)())
        }
        fn cancel(&mut self) {}
        fn product(&self) -> &str {
            HYBRID_PRODUCT
        }
        fn path(&self) -> &str {
            "WebSocket"
        }
    }

    /// 083 H4 review: a connected phone whose connection drops once it was
    /// asked is the third stop another try can fix — said as the phone's
    /// connection dropping, never the core's "The security key stopped
    /// responding. Unplug it…" — and the card comes down. A tunnel the phone
    /// closed is still its own cancel, and no stop at all.
    #[test]
    fn a_phone_that_drops_once_asked_is_worth_another_try() {
        use crate::executor::sign_request::PhoneStop;
        let channel = crate::ceremony::CeremonyChannel::new();
        let ceremony = channel.ceremony(0);
        (ceremony.touch)(Some(phone_prompt(TouchKind::Presence, HYBRID_PRODUCT)));

        let mut silent = cable::PhoneCable(Box::new(Failing(|| CableError::TimedOut)));
        let dropped = assert_over(&mut silent, &ceremony, &[0x11; 32], None)
            .err()
            .unwrap_or_else(|| unreachable!("a silent phone signs nothing"));
        assert!(dropped.phone_dropped(), "{:?}", dropped.message);
        assert_eq!(dropped.phone_stop(), Some(PhoneStop::Dropped));
        assert!(
            dropped
                .message
                .as_deref()
                .is_some_and(|words| !words.contains("security key")),
            "{:?}",
            dropped.message
        );
        assert_eq!(channel.touch_waiting(), None, "no card over the failure");

        let mut hung_up = cable::PhoneCable(Box::new(Failing(|| {
            CableError::Other(format!("caBLE transport: {}", cable::TUNNEL_CLOSED))
        })));
        let closed = assert_over(&mut hung_up, &ceremony, &[0x11; 32], None)
            .err()
            .unwrap_or_else(|| unreachable!("a closed tunnel signs nothing"));
        assert_eq!(
            closed.kind,
            FailureKind::Cancelled,
            "the phone's own cancel"
        );
        assert_eq!(closed.phone_stop(), None);
    }

    /// A cable that is connected and is never asked anything here.
    struct Connected;
    impl Cable for Connected {
        fn exchange(
            &mut self,
            _request: &[u8],
            _touch: Option<TouchKind>,
        ) -> Result<Vec<u8>, CableError> {
            unreachable!("only set up, never asked")
        }
        fn cancel(&mut self) {}
        fn product(&self) -> &str {
            "phone"
        }
        fn path(&self) -> &str {
            "cable"
        }
    }

    /// 083 H5: the QR comes down the moment the phone's advert is found, and
    /// the phone's card goes up in its place for the seconds the tunnel and
    /// the handshake take. The card stays for a phone that connected — its
    /// prompt is next — and goes with a failure or a close made meanwhile.
    #[test]
    fn a_found_phone_takes_the_qr_down_and_says_look_at_the_phone() {
        let channel = crate::ceremony::CeremonyChannel::new();
        let ceremony = channel.ceremony(0);
        let phone = Some(phone_connecting());

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        phone_found(&ceremony);
        assert_eq!(channel.qr_showing(), None, "the QR has done its job");
        assert_eq!(channel.touch_waiting(), phone, "check your phone");
        assert!(
            channel.touch_waiting().is_some_and(|card| card.remote),
            "the phone's words, not a security key's"
        );
        assert!(
            channel.touch_waiting().is_some_and(|card| card.cancellable),
            "a way out while it connects, where the QR's Cancel was"
        );
        assert!(!(ceremony.cancelled)(), "a scan is not a dismissal");
        assert!(hybrid_ready(&ceremony, Ok(Box::new(Connected))).is_ok());
        assert_eq!(channel.touch_waiting(), phone, "its prompt is next");

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        phone_found(&ceremony);
        let broken = hybrid_ready(
            &ceremony,
            Err(HybridError::Handshake(
                "the phone stopped answering".to_owned(),
            )),
        );
        assert!(broken.is_err_and(|failure| failure.phone_link_failed()));
        assert_eq!(channel.touch_waiting(), None, "no card over the failure");

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        phone_found(&ceremony);
        // The column's ✕ while the tunnel comes up (`SigningHost::close`).
        channel.cancel_qr();
        let closed = hybrid_ready(&ceremony, Ok(Box::new(Connected)));
        assert!(closed.is_err_and(|failure| failure.kind == FailureKind::Cancelled));
        assert_eq!(channel.touch_waiting(), None);
    }

    /// A connected phone that counts its goodbyes and must be asked nothing.
    struct Waiting(Arc<std::sync::atomic::AtomicUsize>);
    impl Cable for Waiting {
        fn exchange(
            &mut self,
            _request: &[u8],
            _touch: Option<TouchKind>,
        ) -> Result<Vec<u8>, CableError> {
            unreachable!("a dismissed ceremony asks the phone nothing")
        }
        fn cancel(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        fn product(&self) -> &str {
            HYBRID_PRODUCT
        }
        fn path(&self) -> &str {
            "WebSocket"
        }
    }

    /// 083 H4 review: the connecting card's Cancel stops what it says. The
    /// card comes down at once; a phone that connected meanwhile is asked
    /// nothing and told goodbye; the ceremony comes back a dismissal — and
    /// so it does for a Cancel in the breath between the handshake and the
    /// first request. A QR dismissed in the instant its phone was found
    /// raises no card over the flow the person just left.
    #[test]
    fn the_connecting_card_stops_the_ceremony() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let channel = crate::ceremony::CeremonyChannel::new();
        let ceremony = channel.ceremony(0);
        let goodbyes = Arc::new(AtomicUsize::new(0));

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        phone_found(&ceremony);
        channel.cancel_touch();
        assert_eq!(channel.touch_waiting(), None, "the card comes down at once");
        let cancelled = hybrid_ready(&ceremony, Ok(Box::new(Waiting(Arc::clone(&goodbyes)))));
        assert!(cancelled.is_err_and(|failure| failure.kind == FailureKind::Cancelled));
        assert_eq!(goodbyes.load(Ordering::SeqCst), 1, "told goodbye");

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        phone_found(&ceremony);
        let mut connected = hybrid_ready(&ceremony, Ok(Box::new(Waiting(Arc::clone(&goodbyes)))))
            .unwrap_or_else(|failure| unreachable!("connected: {failure:?}"));
        channel.cancel_touch();
        let late = assert_over(connected.as_mut(), &ceremony, &[0x11; 32], None)
            .err()
            .unwrap_or_else(|| unreachable!("a dismissed ceremony signs nothing"));
        assert_eq!(late.kind, FailureKind::Cancelled);
        assert_eq!(goodbyes.load(Ordering::SeqCst), 2);
        assert_eq!(channel.touch_waiting(), None);

        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        channel.cancel_qr();
        phone_found(&ceremony);
        assert_eq!(channel.qr_showing(), None);
        assert_eq!(
            channel.touch_waiting(),
            None,
            "no card over a cancelled flow"
        );
    }

    /// The clientDataJSON this client signs must be readable by the SAME
    /// parsers the browser path feeds.
    ///
    /// This is the drift that would hurt most and show up latest: a desktop
    /// ceremony that produces a subtly different envelope still signs fine, and
    /// then the core refuses the assertion — or worse, the registry accepts a
    /// proof whose offsets point at the wrong bytes.
    #[test]
    fn the_client_data_is_what_the_core_already_knows_how_to_read() {
        // An assertion's authenticatorData, with UV set — which is what
        // `validate_client_data` checks for on the `get` path.
        let mut authenticator_data = vec![0u8; 32];
        authenticator_data.push(0x05);
        authenticator_data.extend_from_slice(&[0, 0, 0, 0]);

        for kind in [ClientDataKind::Create, ClientDataKind::Get] {
            let json = ceremony::client_data_json(kind, &[0x11; 32], ORIGIN);
            if let Err(error) =
                webauthn::validate_client_data(kind, json.as_bytes(), &authenticator_data)
            {
                unreachable!("the core rejected our own clientDataJSON: {error:?}");
            }
        }
    }

    /// A member proof is assembled by FINDING two substrings in whatever the
    /// authenticator produced, and reporting their byte offsets to the registry
    /// contract. Our envelope has to contain both, in the shape the finder
    /// expects.
    #[test]
    fn a_member_proof_can_be_built_from_our_envelope() {
        let json = ceremony::client_data_json(ClientDataKind::Get, &[0x22; 32], ORIGIN);
        let mut authenticator_data = vec![0u8; 32];
        authenticator_data.push(0x05);
        authenticator_data.extend_from_slice(&[0, 0, 0, 0]);
        // A syntactically valid DER signature is all `build_member_proof` needs
        // to reach the offsets; its VALUE is the authenticator's business.
        let signature = signature_der();

        let proof = match build_member_proof(
            &primitives::to_hex(&authenticator_data, false),
            &primitives::to_hex(json.as_bytes(), false),
            &primitives::to_hex(&signature, false),
        ) {
            Ok(proof) => proof,
            Err(error) => unreachable!("{error:?}"),
        };
        assert_eq!(
            &proof.client_data_json[proof.type_index as usize
                ..proof.type_index as usize + r#""type":"webauthn.get""#.len()],
            r#""type":"webauthn.get""#
        );
        assert_eq!(
            &proof.client_data_json[proof.challenge_index as usize
                ..proof.challenge_index as usize + r#""challenge":""#.len()],
            r#""challenge":""#
        );
    }

    /// Spec 038 finding 18 / SC-427: a phone that hangs up mid-assertion must
    /// take the "check your phone" card down and be reported as a
    /// cancellation — the sheet then names it, and Back works.
    #[test]
    fn hybrid_teardown_clears_the_touch_card_on_a_peer_close() {
        use std::sync::Mutex;

        struct HungUp;
        impl Cable for HungUp {
            fn exchange(
                &mut self,
                _request: &[u8],
                _touch: Option<TouchKind>,
            ) -> Result<Vec<u8>, CableError> {
                Err(CableError::Other(
                    crate::ctap::cable::TUNNEL_CLOSED.to_owned(),
                ))
            }
            fn cancel(&mut self) {
                unreachable!("no goodbye into a closed tunnel");
            }
            fn product(&self) -> &str {
                "phone"
            }
            fn path(&self) -> &str {
                "cable"
            }
        }

        let touches: Arc<Mutex<Vec<bool>>> = Arc::default();
        let seen = Arc::clone(&touches);
        let ceremony = Ceremony {
            touch: Arc::new(move |request| {
                if let Ok(mut log) = seen.lock() {
                    log.push(request.is_some());
                }
            }),
            pin: Arc::new(|_| None),
            pick: Arc::new(|_| None),
            qr: Arc::new(|_| {}),
            cancelled: Arc::new(|| false),
            trusted_signer: crate::executor::trusted_signer::Channel::new().0,
            window: 0,
        };
        // The card is up — the phone had announced a touch.
        (ceremony.touch)(Some(TouchRequest {
            kind: TouchKind::Presence,
            product: "phone".to_owned(),
            remote: true,
            // The phone is mid-ceremony over the tunnel; nothing here polls.
            cancellable: false,
        }));

        let failure = match assert_over(&mut HungUp, &ceremony, &[0x11; 32], None) {
            Err(failure) => failure,
            Ok(_) => unreachable!("a closed tunnel cannot sign"),
        };
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert!(
            failure
                .message
                .as_deref()
                .is_some_and(|m| m.contains("phone"))
        );
        let log = touches.lock().map(|log| log.clone()).unwrap_or_default();
        assert_eq!(
            log.last(),
            Some(&false),
            "the card must come down on the failure path"
        );
    }

    /// A low-S DER signature over a fixed digest, produced here rather than
    /// hard-coded so the vector cannot drift out of the curve.
    fn signature_der() -> Vec<u8> {
        use p256::ecdsa::signature::hazmat::PrehashSigner as _;
        use p256::ecdsa::{Signature, SigningKey};
        let key = match SigningKey::from_slice(&[0x42u8; 32]) {
            Ok(key) => key,
            Err(error) => unreachable!("{error}"),
        };
        let signature: Signature = match key.sign_prehash(&[0x33u8; 32]) {
            Ok(signature) => signature,
            Err(error) => unreachable!("{error}"),
        };
        signature
            .normalize_s()
            .unwrap_or(signature)
            .to_der()
            .to_bytes()
            .to_vec()
    }

    /// Two handles for the same name must differ: the uuid tail is what stops
    /// a second key inheriting the first one's user handle. (The budget and
    /// shape rules are pinned beside the builder, in the core.)
    #[test]
    fn two_handles_for_one_name_differ() {
        assert_ne!(
            user_handle("Everyday wallet"),
            user_handle("Everyday wallet")
        );
    }

    /// "No key plugged in" is `not_supported` WITH WORDS, all the way through
    /// this shell's mapping chain — the kind because the core branches on it,
    /// the message because on a desktop this is the one failure that is an
    /// instruction, not an error.
    #[test]
    fn a_missing_key_is_a_sentence_rather_than_a_shrug() {
        let failure = device_failure(UsbError::NoKeyPresent);
        assert_eq!(failure.kind, FailureKind::NotSupported);
        let message = failure.message.unwrap_or_default();
        assert!(
            message.to_lowercase().contains("security key"),
            "the message must name what is missing: {message}"
        );
    }

    /// A person who declines at the authenticator has not hit an error.
    /// Cancellation carries no message — its copy comes from the
    /// classification, and forwarding "the security key refused" would tell
    /// somebody their own choice went wrong.
    #[test]
    fn declining_at_the_key_is_a_cancellation_with_no_words() {
        let failure = device_failure(UsbError::Ctap(Status::Cancelled));
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert_eq!(failure.message, None);
    }

    /// The exclusion list doing its job is not a fault, and it is not
    /// `not_supported` either: a different key would work.
    #[test]
    fn an_excluded_credential_says_to_use_another_key() {
        let failure = device_failure(UsbError::Ctap(Status::CredentialExcluded));
        assert_eq!(failure.kind, FailureKind::Other);
        assert!(
            failure
                .message
                .unwrap_or_default()
                .to_lowercase()
                .contains("different")
        );
    }

    /// A key with no PIN and no biometric cannot make a wallet key, and the
    /// sentence says what to do about it — the brand-new-key case, which a
    /// person cannot guess at from `CTAP2_ERR_UNSUPPORTED_OPTION`.
    #[test]
    fn a_key_with_no_pin_gets_an_instruction() {
        let failure = device_failure(UsbError::Ctap(Status::PinNotSet));
        assert_eq!(failure.kind, FailureKind::Other);
        let message = failure.message.unwrap_or_default();
        assert!(message.contains("no PIN set"), "{message}");
        assert!(
            message.contains("try again"),
            "the sentence has to end in something to do: {message}"
        );
    }

    /// A refused PIN and a locked key are different sentences, and telling a
    /// person their key is locked when they merely mistyped is the failure
    /// mode worth a test: a reset is the only way out of the locked state, and
    /// a reset destroys the wallet's founding credential.
    #[test]
    fn a_wrong_pin_and_a_locked_key_do_not_share_a_sentence() {
        let wrong = device_failure(UsbError::Ctap(Status::PinRequired))
            .message
            .unwrap_or_default();
        let locked = device_failure(UsbError::Ctap(Status::PinBlocked))
            .message
            .unwrap_or_default();
        assert!(wrong.contains("not accepted"), "{wrong}");
        assert!(!wrong.contains("locked"), "{wrong}");
        assert!(locked.contains("locked"), "{locked}");
        assert!(
            locked.contains("erases"),
            "a reset destroys every passkey on the key, and that has to be said: {locked}"
        );
    }

    /// Support means the HID subsystem is REACHABLE, not that a key is plugged
    /// in — and the difference is the whole recovery path. Answering `false`
    /// for an empty USB port would make the core raise "this device cannot
    /// create a wallet", which is untrue and has no way back; a missing key has
    /// to arrive later, as a failure the person can fix by plugging one in.
    ///
    /// So this asserts support on a machine that (almost certainly) has no
    /// security key attached, which is exactly the case that must not report
    /// unsupported.
    #[test]
    fn support_does_not_depend_on_a_key_being_plugged_in() {
        assert!(
            supported(),
            "HID enumeration is unavailable on this host, so this test cannot say anything"
        );
    }
}

// ---------------------------------------------------------------------------
// Against real hardware
// ---------------------------------------------------------------------------

#[cfg(test)]
mod hardware_tests {
    use super::*;

    /// The full ceremony round trip, against a plugged-in authenticator.
    ///
    /// `#[ignore]` because it needs hardware AND a finger:
    ///
    /// ```bash
    /// VELA_TEST_PIN=<your key's PIN> \
    ///   cargo test -p vela-wallet register_then_assert -- --ignored --nocapture
    /// ```
    ///
    /// The PIN comes from the environment because it is a secret that belongs to
    /// the person at the desk, not to this repository. Omit it for a key with no
    /// PIN set — the request returns `None` and the ceremony either proceeds
    /// without user verification or fails saying so, which is itself the third
    /// hardware state `results.md` asks about.
    ///
    /// **What this proves that `a_plugged_in_key_answers_get_info` cannot**: the
    /// PIN/UV auth protocol, `makeCredential` with an exclude list, attestation
    /// parsing, `getAssertion` against a named credential, and — the claim T089
    /// is actually about — that signing in costs **one** touch. The assertion
    /// below counts them.
    #[test]
    #[ignore]
    fn register_then_assert() {
        let touches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&touches);
        let ceremony = Ceremony {
            touch: Arc::new(move |request| {
                if let Some(request) = request {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    eprintln!("  >>> TOUCH YOUR KEY ({:?}) <<<", request.kind);
                }
            }),
            qr: Arc::new(|_| {}),
            cancelled: Arc::new(|| false),
            trusted_signer: crate::executor::trusted_signer::Channel::new().0,
            pin: Arc::new(|request| {
                let pin = std::env::var("VELA_TEST_PIN").ok();
                eprintln!(
                    "  key asked for a PIN (retries left: {:?}); {}",
                    request.retries,
                    if pin.is_some() {
                        "supplying VELA_TEST_PIN"
                    } else {
                        "none set"
                    }
                );
                pin
            }),
            // One key, one credential: a picker that had to choose would mean
            // the test is running against a state it did not set up.
            pick: Arc::new(|choices| {
                assert_eq!(
                    choices.len(),
                    1,
                    "expected exactly one credential to choose from"
                );
                Some(0)
            }),
            window: 0,
        };

        let registration = register("Hardware test", &[], KeyMethod::SecurityKey, &ceremony)
            .expect("makeCredential against the plugged-in key");
        let after_register = touches.load(std::sync::atomic::Ordering::SeqCst);
        eprintln!("registered credential {}", registration.credential_id);

        // The attestation must yield a P-256 key, or nothing downstream works:
        // the address derivation, the on-chain verifier and two-signature
        // recovery are all ES256.
        let attestation = vela_core::primitives::from_hex(&registration.attestation_object_hex)
            .expect("attestation is hex");
        let key = vela_core::webauthn::extract_attestation_public_key(&attestation)
            .expect("a P-256 public key comes out of the attestation");
        assert_eq!(key.x.len(), 32);
        assert_eq!(key.y.len(), 32);

        // The claim T089 is about: signing in costs ONE touch, not two. Two
        // means the common path regressed into the recovery flow, which asks
        // for a second signature to pin down the key.
        touches.store(0, std::sync::atomic::Ordering::SeqCst);
        let assertion = assert(
            b"vela-hardware-test-challenge",
            Some(&registration.credential_id),
            KeyMethod::SecurityKey,
            &ceremony,
        )
        .expect("getAssertion against the credential just minted");
        let for_assertion = touches.load(std::sync::atomic::Ordering::SeqCst);

        assert_eq!(
            assertion.credential_id, registration.credential_id,
            "the key signed with a different credential than the one asked for"
        );
        assert!(
            !assertion.signature_der_hex.is_empty(),
            "no signature came back"
        );
        assert_eq!(
            for_assertion, 1,
            "signing in asked for {for_assertion} touches; T089 says it must be exactly one \
             (two means the common path regressed to recovery)"
        );
        eprintln!(
            "OK — register took {after_register} touch prompt(s), assert took {for_assertion}"
        );
    }

    /// The exclude list, against the same authenticator.
    ///
    /// ```bash
    /// VELA_TEST_PIN=… cargo test excluded_credential_is_refused -- --ignored --nocapture
    /// ```
    ///
    /// **Why this matters more than it looks.** A multi-key wallet registers each
    /// founding key separately, and the Safe address is derived from ALL of them.
    /// If the second registration silently REPLACED the first on the same
    /// authenticator, the person would end up with one credential where the core
    /// believes there are two — and the address it derived belongs to a key set
    /// that no longer exists. Nothing on screen would look wrong; the wallet
    /// would simply be unreachable and unfundable.
    ///
    /// So the authenticator MUST refuse, and this is the only test that proves a
    /// real one does. It needs one key, not two: registering twice on the same
    /// device with the first credential excluded is exactly the case.
    #[test]
    #[ignore]
    fn excluded_credential_is_refused() {
        let ceremony = Ceremony {
            touch: Arc::new(|request| {
                if let Some(request) = request {
                    eprintln!("  >>> TOUCH YOUR KEY ({:?}) <<<", request.kind);
                }
            }),
            qr: Arc::new(|_| {}),
            cancelled: Arc::new(|| false),
            trusted_signer: crate::executor::trusted_signer::Channel::new().0,
            pin: Arc::new(|_| std::env::var("VELA_TEST_PIN").ok()),
            pick: Arc::new(|_| Some(0)),
            window: 0,
        };

        let first = register("Exclude test", &[], KeyMethod::SecurityKey, &ceremony)
            .expect("the first registration");
        eprintln!("first credential: {}", first.credential_id);

        eprintln!("  now registering again with that credential EXCLUDED — the key must refuse");
        let second = register(
            "Exclude test 2",
            std::slice::from_ref(&first.credential_id),
            KeyMethod::SecurityKey,
            &ceremony,
        );

        match second {
            Err(failure) => {
                // The message is the shell's, and the sheet renders it. It has to
                // say what happened, because "try again" would be advice that
                // cannot work on this key.
                eprintln!("refused, as it must: {failure:?}");
                assert_eq!(
                    failure.kind,
                    FailureKind::Other,
                    "an excludeList refusal is not a cancellation and not an \
                     unsupported device — the person can still add a key, on a \
                     DIFFERENT authenticator"
                );
            }
            Ok(second) => panic!(
                "the authenticator minted a SECOND credential ({}) despite the first \
                 ({}) being in excludeCredentials — a multi-key wallet built on this \
                 key would derive an address from a key set that does not exist",
                second.credential_id, first.credential_id
            ),
        }
    }
}
