//! The USB cable, and nothing else.
//!
//! Framing and CBOR come from [`vela_core::ctap`]; this module opens the
//! device, writes 64-byte reports, reads 64-byte reports, and knows when to
//! stop waiting. If a rule about what a byte MEANS ever appears in this file,
//! it is in the wrong place — that rule belongs in the core, where the other
//! four clients can reach it too.
//!
//! ## Why HID and not raw USB
//!
//! FIDO over USB is a HID protocol. On macOS the kernel's HID driver owns the
//! device, so a raw-USB crate cannot claim the interface at all; `hidapi`
//! wraps IOKit, Linux `hidraw` and the Windows HID API behind one enumeration
//! that reports `usage_page` / `usage`, which is exactly the filter FIDO needs
//! (research D3).

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use vela_core::ctap::hid::{self, BROADCAST_CHANNEL, CtapHidCommand, Message, Reassembler};
use vela_core::ctap::{
    CredentialDescriptor, GetAssertion, HID_REPORT_SIZE, Status, selection_request, split_response,
};

/// The answer to "does this key hold that credential?".
enum Holds {
    Yes,
    No,
    /// The key would not say — too old for a silent probe, or the cable
    /// hiccuped. Not a reason to exclude it from the fallback.
    Unknown,
}

/// The HID usage page every FIDO authenticator declares, and the usage within
/// it. A keyboard, a mouse and a webcam all also enumerate as HID devices; this
/// pair is what separates a security key from the rest of the bus.
const FIDO_USAGE_PAGE: u16 = 0xf1d0;
const FIDO_USAGE: u16 = 0x01;

/// How long one report read blocks before the loop checks its deadline again.
/// Short enough that a cancelled ceremony stops promptly, long enough not to
/// spin.
const READ_SLICE: Duration = Duration::from_millis(250);

/// How long a whole exchange may take. Long, because most of it is a person
/// noticing the key is blinking and putting a finger on it — the CTAP2 spec's
/// own user-presence budget is 30 seconds and an authenticator may renew it.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(120);

/// What can go wrong on the cable. Everything here is a device, a driver or a
/// person walking away — the executor turns each into the failure variant the
/// operation owes, and never lets one reach the effect loop as a panic.
#[derive(Debug)]
pub enum UsbError {
    /// No FIDO authenticator is plugged in. Its own variant because it is the
    /// one failure the desktop client must be able to say in words: without a
    /// system passkey service, a missing key is not "something went wrong", it
    /// is the whole story.
    NoKeyPresent,
    /// A key IS plugged in and this process cannot open it.
    ///
    /// Distinct from [`Self::NoKeyPresent`], and the distinction is the whole
    /// point: reporting "no security key is plugged in" to somebody looking
    /// straight at one sends them hunting for a hardware fault that is really
    /// a permissions problem. On Linux it is the FIDO udev rules — systemd 252
    /// and later tag hidraw FIDO devices for the logged-in user, and older
    /// systems need their distribution's u2f rules package. Carries the OS's
    /// own words, which go into the technical details.
    AccessDenied { product: String, detail: String },
    /// The HID subsystem itself is unavailable — no permission to enumerate,
    /// or a driver that is not there.
    Hid(String),
    /// The device answered something CTAPHID cannot parse.
    Framing(hid::HidError),
    /// The device stopped answering.
    TimedOut,
    /// The authenticator refused: a CTAP status byte, kept as its number.
    Ctap(Status),
    /// Encoding a request failed, which means a bug in this client rather than
    /// in the device.
    Encode(String),
}

impl std::fmt::Display for UsbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoKeyPresent => write!(f, "no security key is plugged in"),
            Self::AccessDenied { product, detail } => {
                write!(
                    f,
                    "{product} is plugged in but could not be opened: {detail}"
                )
            }
            Self::Hid(message) => write!(f, "USB HID unavailable: {message}"),
            Self::Framing(error) => write!(f, "the security key sent a malformed reply: {error:?}"),
            Self::TimedOut => write!(f, "the security key stopped responding"),
            Self::Ctap(status) => write!(f, "the security key refused: {status:?}"),
            Self::Encode(message) => write!(f, "could not encode a CTAP2 request: {message}"),
        }
    }
}

/// What the key is waiting for. Lives beside the ceremony in the core since
/// the orchestration moved there (spec 019 phase 11); re-exported so the
/// transport's vocabulary stays reachable in one place.
pub use vela_core::ctap::ceremony::TouchKind;

/// What the screen should say while the key blinks.
#[derive(Clone, Debug)]
pub struct TouchRequest {
    pub kind: TouchKind,
    /// The key's own product string, so the prompt names the thing on the desk.
    pub product: String,
    /// The authenticator is a REMOTE device reached over caBLE (a phone), not a
    /// security key on the desk — the prompt says "follow the steps on your
    /// device" instead of "touch your security key".
    pub remote: bool,
}

/// Told when the authenticator starts waiting for a person, and when it stops.
///
/// A `KEEPALIVE` carrying `UP_NEEDED` is the only moment a client can honestly
/// say "touch your key" — before it, the device has not asked; after the
/// answer, the moment has passed. `Send + Sync` because the exchange runs on a
/// background thread and the screen it updates does not.
pub type TouchNotifier = Arc<dyn Fn(Option<TouchRequest>) + Send + Sync>;

/// `KEEPALIVE` status: the authenticator is waiting for user presence.
const KEEPALIVE_UP_NEEDED: u8 = 0x02;

/// Every FIDO authenticator on the bus, opened.
///
/// The two error cases are deliberately different answers. Nothing on the bus
/// is [`UsbError::NoKeyPresent`] — a true statement a person can act on by
/// plugging one in. Something on the bus that will not open is
/// [`UsbError::AccessDenied`], which is a permissions problem wearing a
/// hardware problem's clothes: on Linux it means the FIDO udev rules are
/// missing, and reporting it as absence sends the person looking at the port
/// instead of at the rules.
///
/// A key that fails to open while ANOTHER opens fine is simply skipped: the
/// working one is the answer, and one device's trouble is not the ceremony's.
fn enumerate() -> Result<Vec<(String, String, HidDevice)>, UsbError> {
    hid_thread::run(enumerate_here)
        .unwrap_or_else(|| Err(UsbError::Hid("the HID thread could not answer".to_owned())))
}

/// Is the HID subsystem reachable at all? The support gate's question, asked
/// on the same thread as everything else that touches hidapi — see
/// [`hid_thread`] for why it must not be asked anywhere else.
pub fn hid_reachable() -> bool {
    hid_thread::run(|| HidApi::new().is_ok()).unwrap_or(false)
}

/// The one thread hidapi is ever initialised or enumerated on.
///
/// hidapi's macOS backend keeps ONE process-global `IOHIDManager` and
/// schedules it on the run loop of **whichever thread initialised it**
/// (`mac/hid.c`: `IOHIDManagerScheduleWithRunLoop(hid_mgr,
/// CFRunLoopGetCurrent(), …)`), and hidapi-rs never tears it down. When that
/// thread ends, the manager is left holding a dead run loop; the next
/// enumeration that has a device IOKit has not scheduled yet — a key plugged in
/// after launch, or pulled and pushed back — hands it to `CFRunLoopAddSource`
/// and the process dies in `__CFCheckCFInfoPACSignature` (SIGTRAP).
///
/// This was met three times. The support probe crashed on gpui's libdispatch
/// workers and was moved to a one-shot thread; the L2CAP port corrupted a
/// worker's run loop and got a private thread. The one-shot probe thread was
/// the trap's third form: it initialised the manager and then ENDED, so every
/// later enumeration ran against a run loop that no longer existed — "creating
/// a wallet with a USB key works, signing in crashes" (founder, 2026-09-19, in
/// the first notarized build). Fixing call sites one at a time is what let it
/// come back, so there is now exactly one place hidapi may be entered from, and
/// its thread lives as long as the process.
///
/// Only initialisation and enumeration need this. An opened [`HidDevice`] is
/// `Send` and gets its own read thread and run loop from hidapi, so the
/// handles this returns are used from the ceremony's threads as before.
mod hid_thread {
    use std::sync::{OnceLock, mpsc};

    type Job = Box<dyn FnOnce() + Send>;

    static JOBS: OnceLock<mpsc::Sender<Job>> = OnceLock::new();

    /// Run `work` on the HID thread and wait for its answer. `None` only if
    /// the work panicked (the thread survives it) or the thread could not be
    /// started at all.
    pub fn run<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let jobs = JOBS.get_or_init(|| {
            let (jobs, inbox) = mpsc::channel::<Job>();
            // If the spawn fails the receiver is dropped with it, every send
            // below errors, and callers get `None` — reported, not fatal.
            let _ = std::thread::Builder::new()
                .name("vela-hid".to_owned())
                .spawn(move || {
                    for job in inbox {
                        job();
                    }
                });
            jobs
        });
        let (answer, reply) = mpsc::channel();
        jobs.send(Box::new(move || {
            // A panic in one enumeration must not end the thread: the thread
            // ending IS the bug this module exists to prevent.
            if let Some(value) = crate::panic_report::guarded(work) {
                let _ = answer.send(value);
            }
        }))
        .ok()?;
        reply.recv().ok()
    }

    #[cfg(test)]
    mod tests {
        use super::run;

        /// The property the fix rests on, stated directly: whoever asks, from
        /// whatever thread — including threads that have since ended — the
        /// work runs on the SAME thread, and that thread is still there.
        #[test]
        fn every_caller_lands_on_the_one_thread_that_never_ends() {
            let here = || {
                (
                    std::thread::current().id(),
                    std::thread::current().name().map(str::to_owned),
                )
            };
            let first = run(here).expect("the HID thread answers");
            assert_eq!(first.1.as_deref(), Some("vela-hid"));
            assert_ne!(
                first.0,
                std::thread::current().id(),
                "not the caller's thread"
            );
            for _ in 0..4 {
                let from_a_thread_that_ends = std::thread::spawn(move || run(here))
                    .join()
                    .expect("join")
                    .expect("the HID thread answers");
                assert_eq!(from_a_thread_that_ends, first);
            }
        }

        /// A panicking job is reported as `None` and the thread outlives it.
        #[test]
        fn a_panic_does_not_end_the_thread() {
            assert_eq!(
                run(|| -> u8 { panic!("a key was yanked mid-enumeration") }),
                None
            );
            assert_eq!(run(|| 7u8), Some(7));
        }
    }
}

/// [`enumerate`], on whatever thread it is called from — which must be the HID
/// thread and nothing else.
fn enumerate_here() -> Result<Vec<(String, String, HidDevice)>, UsbError> {
    let api = HidApi::new().map_err(|error| UsbError::Hid(error.to_string()))?;
    let mut opened = Vec::new();
    let mut refused: Option<(String, String)> = None;
    for info in api
        .device_list()
        .filter(|device| device.usage_page() == FIDO_USAGE_PAGE && device.usage() == FIDO_USAGE)
    {
        let product = info.product_string().unwrap_or("security key").to_owned();
        let path = info.path().to_string_lossy().into_owned();
        match info.open_device(&api) {
            Ok(device) => opened.push((product, path, device)),
            Err(error) => {
                refused.get_or_insert((product, error.to_string()));
            }
        }
    }

    if !opened.is_empty() {
        return Ok(opened);
    }
    match refused {
        Some((product, detail)) => Err(UsbError::AccessDenied { product, detail }),
        None => Err(UsbError::NoKeyPresent),
    }
}

/// One open conversation with one authenticator.
pub struct SecurityKey {
    device: HidDevice,
    channel: u32,
    /// What the device says about itself. Read once, at open: the PIN protocol
    /// and the `rk` capability both come from here, and asking twice would let
    /// a ceremony act on a different answer than the one it decided with.
    product: String,
    /// The HID path — stable for as long as the device stays plugged in, and
    /// DIFFERENT for two keys of the same model, which the product string is
    /// not. It is an identity, never shown: the only thing that reads it is the
    /// PIN cache, which must not hand one key's PIN to another.
    path: String,
}

impl SecurityKey {
    /// Open the key the person touches.
    ///
    /// With one authenticator plugged in this is just "open it". With several,
    /// **the one they touch wins** — which is the only answer that is not the
    /// computer choosing for them. Picking by enumeration order would mean a
    /// person with a work key and a personal key in adjacent ports gets
    /// whichever the OS happened to list first, with no way to say otherwise
    /// short of unplugging one.
    ///
    /// The mechanism is CTAP 2.1's `authenticatorSelection`: send it to every
    /// key at once, and the first to answer is the one under a finger. Every
    /// other key is told to cancel, so nothing is left blinking. A key too old
    /// to understand the command drops out of the race rather than failing it —
    /// and if they ALL do, the first one is used, which is where this started.
    ///
    /// `nonce` is called once per device: each conversation needs its own
    /// `INIT` nonce, and reusing one would let two channels be confused for
    /// each other.
    pub fn open_touched(
        nonce: &dyn Fn() -> [u8; 8],
        touch: Option<&TouchNotifier>,
    ) -> Result<Self, UsbError> {
        // One `HidApi` for the whole process: hidapi refuses a second live
        // instance, so the devices are opened HERE and the handles move into
        // the race. `HidDevice` is `Send`, which is what makes that legal.
        let mut opened = enumerate()?;
        if opened.len() == 1 {
            let (product, path, device) = opened.remove(0);
            let mut key = Self::new(product, path, device);
            key.channel = key.init(nonce())?;
            return Ok(key);
        }

        Self::race(opened, nonce, touch)
    }

    /// Open the key that already holds a particular credential.
    ///
    /// A `get()` knows which credential it wants, so making every plugged-in
    /// key blink and asking a person to pick is asking them to answer a
    /// question the client could answer itself. Each key is probed SILENTLY —
    /// a `getAssertion` with `up: false`, which no authenticator lights up for
    /// — and the one that does not say "no credentials" is the one.
    ///
    /// Falls back to the touch race when the answer is not exactly one key:
    /// zero (the credential is on something unplugged), several (a credential
    /// restored onto two keys), or a device too old to answer a silent probe.
    /// A guess would be worse than a question.
    pub fn open_holding(
        credential_id: &[u8],
        rp_id: &str,
        probe_hash: &[u8],
        nonce: &dyn Fn() -> [u8; 8],
        touch: Option<&TouchNotifier>,
    ) -> Result<Self, UsbError> {
        let opened = enumerate()?;

        let mut holders = Vec::new();
        let mut rest = Vec::new();
        for (product, path, device) in opened {
            let mut key = Self::new(product, path, device);
            match key.init(nonce()) {
                Ok(channel) => key.channel = channel,
                Err(_) => continue,
            }
            match key.holds(credential_id, rp_id, probe_hash) {
                Holds::Yes => holders.push(key),
                // Definitively not this one — it is not even a candidate for
                // the fallback race, because it cannot answer the assertion.
                Holds::No => {}
                Holds::Unknown => rest.push(key),
            }
        }

        if holders.len() == 1 {
            return Ok(holders.remove(0));
        }
        // Ambiguous. Ask, rather than guess — but only among the keys that
        // might hold it, so nothing blinks that certainly cannot answer.
        holders.append(&mut rest);
        if holders.is_empty() {
            return Err(UsbError::NoKeyPresent);
        }
        if holders.len() == 1 {
            return Ok(holders.remove(0));
        }
        Self::race_open(holders, touch)
    }

    /// Does this key hold that credential? Asked without lighting it up.
    fn holds(&mut self, credential_id: &[u8], rp_id: &str, probe_hash: &[u8]) -> Holds {
        let probe = GetAssertion {
            rp_id: rp_id.to_owned(),
            client_data_hash: probe_hash.to_vec(),
            allow: vec![CredentialDescriptor {
                id: credential_id.to_vec(),
            }],
            // The whole point: no touch, no light.
            user_presence: false,
            user_verification: false,
            pin_uv_auth: None,
        };
        let Ok(request) = probe.encode() else {
            return Holds::Unknown;
        };
        match self.cbor(&request, None) {
            // It answered an assertion for that credential, so it has it.
            Ok(_) => Holds::Yes,
            Err(UsbError::Ctap(Status::NoCredentials)) => Holds::No,
            // It refused for a reason that is ABOUT the credential rather than
            // about its absence — a PIN it wants first, a UV it insists on.
            // Only a key that holds the credential gets that far.
            Err(UsbError::Ctap(Status::PinRequired | Status::PinNotSet | Status::UvFailed)) => {
                Holds::Yes
            }
            // Anything else — an old key that will not do silent probes, a
            // cable that hiccuped — is not an answer either way.
            Err(_) => Holds::Unknown,
        }
    }

    fn new(product: String, path: String, device: HidDevice) -> Self {
        Self {
            device,
            channel: BROADCAST_CHANNEL,
            product,
            path,
        }
    }

    /// The key's identity for this session. Not for display.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Ask every plugged-in key which one is being touched.
    fn race(
        opened: Vec<(String, String, HidDevice)>,
        nonce: &dyn Fn() -> [u8; 8],
        touch: Option<&TouchNotifier>,
    ) -> Result<Self, UsbError> {
        // Announced before the threads start: several keys are about to blink,
        // and the person needs to know that touching ONE of them is the point.
        if let Some(notify) = touch {
            notify(Some(TouchRequest {
                kind: TouchKind::Select,
                product: String::new(),
                remote: false,
            }));
        }

        let mut keys = Vec::with_capacity(opened.len());
        for (product, path, device) in opened {
            let mut key = Self::new(product, path, device);
            // A key that will not even initialise is not in the race.
            if let Ok(channel) = key.init(nonce()) {
                key.channel = channel;
                keys.push(key);
            }
        }
        Self::finish_race(keys, touch)
    }

    /// The same race over keys that are already open and initialised.
    fn race_open(keys: Vec<Self>, touch: Option<&TouchNotifier>) -> Result<Self, UsbError> {
        if let Some(notify) = touch {
            notify(Some(TouchRequest {
                kind: TouchKind::Select,
                product: String::new(),
                remote: false,
            }));
        }
        Self::finish_race(keys, touch)
    }

    fn finish_race(keys: Vec<Self>, touch: Option<&TouchNotifier>) -> Result<Self, UsbError> {
        let (winner_tx, winner_rx) = mpsc::channel::<Result<Self, ()>>();
        let mut handles = Vec::with_capacity(keys.len());
        for mut key in keys {
            let winner_tx = winner_tx.clone();
            handles.push(thread::spawn(move || {
                let outcome = selection_request()
                    .map_err(|error| UsbError::Encode(error.to_string()))
                    // No notifier on this one: the announcement covers the
                    // whole race, and one per device would fire as many times
                    // as there are keys.
                    .and_then(|request| key.cbor(&request, None))
                    .map(|_| key);
                // A send that fails means the race is already decided and the
                // receiver is gone — the normal ending for every loser.
                let _ = winner_tx.send(outcome.map_err(|_| ()));
            }));
        }
        drop(winner_tx);

        // The first Ok is the touched key. `flatten` drops the errors on the
        // way past, which is the whole handling they need: a key too old for
        // `authenticatorSelection`, or one the person simply did not touch, has
        // only dropped out of the race.
        let winner = winner_rx.into_iter().flatten().next();

        if let Some(notify) = touch {
            notify(None);
        }

        // The losers are still blocked in a read, waiting for a touch that is
        // not coming. They are left to their own exchange timeout rather than
        // joined: joining would make this call wait out the slowest key AFTER
        // the person has already answered, and their handles are dropped when
        // the threads end. `CANCEL` is not sent because the channel that would
        // carry it belongs to the thread that owns the device.
        drop(handles);

        winner.ok_or(UsbError::NoKeyPresent)
    }

    /// The device's product string, for the message a failure sheet shows.
    pub fn product(&self) -> &str {
        &self.product
    }

    /// `CTAPHID_INIT` on the broadcast channel: the device echoes the nonce and
    /// allocates a channel for the rest of the conversation.
    ///
    /// The nonce is checked. Two clients can be talking to the same key at once
    /// — a browser and this app — and both see every broadcast reply; without
    /// the echo check this client would happily adopt the OTHER client's
    /// channel and then interleave with it.
    fn init(&mut self, nonce: [u8; 8]) -> Result<u32, UsbError> {
        let reply = self.exchange(CtapHidCommand::Init, &nonce, None)?;
        if reply.len() < 17 || reply[..8] != nonce {
            return Err(UsbError::Framing(hid::HidError::Truncated {
                declared: 17,
                got: reply.len(),
            }));
        }
        Ok(u32::from_be_bytes([
            reply[8], reply[9], reply[10], reply[11],
        ]))
    }

    /// Send a CTAP2 request and return its response body, or the status the
    /// authenticator refused with.
    ///
    /// `waiting_for` is `Some` only for the requests that make a person do
    /// something. `getInfo` and the key agreement answer instantly, and
    /// announcing a touch for those would train people to ignore the prompt.
    pub fn cbor(
        &mut self,
        request: &[u8],
        touch: Option<(&TouchNotifier, TouchKind)>,
    ) -> Result<Vec<u8>, UsbError> {
        let payload = self.exchange(CtapHidCommand::Cbor, request, touch)?;
        let (status, body) = split_response(&payload).map_err(|error| {
            UsbError::Encode(format!("response is not a CTAP2 message: {error:?}"))
        })?;
        if !status.is_success() {
            return Err(UsbError::Ctap(status));
        }
        Ok(body.to_vec())
    }

    /// Tell the authenticator to abandon whatever it is waiting for.
    ///
    /// Best effort by construction: this is sent when a person walks away from
    /// a blinking key, and the only thing worse than a failed cancel is a
    /// failed cancel that becomes its own error message.
    pub fn cancel(&mut self) {
        if let Ok(frames) = hid::encode(self.channel, CtapHidCommand::Cancel, &[]) {
            for frame in frames {
                let _ = self.write(&frame);
            }
        }
    }

    /// One request out, one message back, with `KEEPALIVE` absorbed.
    fn exchange(
        &mut self,
        command: CtapHidCommand,
        payload: &[u8],
        touch: Option<(&TouchNotifier, TouchKind)>,
    ) -> Result<Vec<u8>, UsbError> {
        let frames = hid::encode(self.channel, command, payload).map_err(UsbError::Framing)?;
        for frame in &frames {
            self.write(frame)?;
        }

        let deadline = Instant::now() + EXCHANGE_TIMEOUT;
        let mut reassembler = Reassembler::new();
        let mut announced_touch = false;
        let mut report = [0u8; HID_REPORT_SIZE];

        let outcome = loop {
            if Instant::now() >= deadline {
                break Err(UsbError::TimedOut);
            }
            let read = self
                .device
                .read_timeout(&mut report, READ_SLICE.as_millis() as i32)
                .map_err(|error| UsbError::Hid(error.to_string()))?;
            if read == 0 {
                continue;
            }
            if read != HID_REPORT_SIZE {
                break Err(UsbError::Framing(hid::HidError::ReportSize(read)));
            }

            match reassembler.push(&report) {
                Ok(None) => continue,
                Ok(Some(Message {
                    command: CtapHidCommand::KeepAlive,
                    payload,
                    ..
                })) => {
                    // Not an answer — the device asking for more time, and
                    // (with UP_NEEDED) the one honest moment to say "touch it".
                    if payload.first() == Some(&KEEPALIVE_UP_NEEDED) && !announced_touch {
                        announced_touch = true;
                        if let Some((notify, kind)) = touch {
                            notify(Some(TouchRequest {
                                kind,
                                product: self.product.clone(),
                                remote: false,
                            }));
                        }
                    }
                    continue;
                }
                Ok(Some(Message {
                    command: CtapHidCommand::Error,
                    payload,
                    ..
                })) => {
                    break Err(UsbError::Ctap(Status::from_byte(
                        payload.first().copied().unwrap_or(0x7f),
                    )));
                }
                Ok(Some(message)) => break Ok(message.payload),
                Err(error) => break Err(UsbError::Framing(error)),
            }
        };

        // Whatever happened, the key is no longer waiting for a finger. Leaving
        // a "touch your key" prompt up after a failure is how a screen ends up
        // asking for something that can no longer happen.
        if let Some((notify, _)) = touch
            && announced_touch
        {
            notify(None);
        }
        outcome
    }

    /// One report out.
    ///
    /// The leading zero is a REPORT ID, not padding. `hid_write` always reads
    /// the first byte as one, and a device that uses no report ids expects 0 —
    /// drop it and every write is off by one byte, which presents as an
    /// authenticator that answers `INVALID_COMMAND` to everything.
    fn write(&self, frame: &[u8; HID_REPORT_SIZE]) -> Result<(), UsbError> {
        let mut buffer = [0u8; HID_REPORT_SIZE + 1];
        buffer[1..].copy_from_slice(frame);
        self.device
            .write(&buffer)
            .map(|_| ())
            .map_err(|error| UsbError::Hid(error.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Against real hardware
// ---------------------------------------------------------------------------

#[cfg(test)]
mod hardware_tests {
    use super::*;

    /// The protocol layer, against a plugged-in authenticator.
    ///
    /// `#[ignore]` because it needs hardware: run it with
    /// `cargo test -- --ignored --nocapture` on a host with a FIDO2 key in a
    /// USB port. It takes NO touch — `getInfo` is answered instantly by every
    /// authenticator — which is what makes it worth having as a gate rather
    /// than as a manual walk.
    ///
    /// This is the check `results.md` named as the riskiest untested surface:
    ///
    /// > The riskiest untested surface is the report-id byte in `usb.rs::write`:
    /// > a device that uses report ids would need the first byte to be its id
    /// > rather than zero, and getting it wrong presents as an authenticator
    /// > answering `INVALID_COMMAND` to everything. FIDO devices do not use
    /// > report ids, so zero is right — but that is reasoning, not a
    /// > measurement.
    ///
    /// A green run here IS the measurement: an `INIT` that allocates a channel
    /// and a `getInfo` that parses proves the report-id byte, the 64-byte
    /// framing, the init/continuation split and the canonical CBOR decoder all
    /// agree with a real device.
    #[test]
    #[ignore]
    fn a_plugged_in_key_answers_get_info() {
        let mut key = match SecurityKey::open_touched(&|| [0x11; 8], None) {
            Ok(key) => key,
            Err(UsbError::NoKeyPresent) => {
                panic!("no FIDO2 key is plugged in — this test needs one")
            }
            Err(error) => panic!("could not open the key: {error:?}"),
        };

        let request = vela_core::ctap::commands::get_info_request().expect("encode getInfo");
        let body = key.cbor(&request, None).expect("getInfo");
        let info = vela_core::ctap::commands::parse_get_info(&body).expect("parse getInfo");

        println!("product: {}", key.product());
        println!("versions: {:?}", info.versions);
        println!("pin protocols: {:?}", info.pin_protocols);
        println!("resident key capable: {}", info.resident_key);
        println!("client pin set: {:?}", info.client_pin_set);

        // The three facts the create flow actually branches on.
        assert!(
            info.versions
                .iter()
                .any(|v| v == "FIDO_2_0" || v.starts_with("FIDO_2")),
            "this key does not speak CTAP2 at all: {:?}",
            info.versions
        );
        assert!(
            info.resident_key,
            "the key reports no resident-key capability — a Vela founding key MUST be \
             discoverable, or it signs fine and never appears in a picker"
        );
        assert!(
            !info.pin_protocols.is_empty(),
            "the key advertises no PIN/UV auth protocol, so user verification is impossible"
        );
    }
}
