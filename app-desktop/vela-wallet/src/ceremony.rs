//! The two things a ceremony has to say to a screen, and the one thing a screen
//! has to say back.
//!
//! A CTAP2 ceremony runs on a background thread: it opens a device, waits for a
//! finger, does TLS. Two moments inside it belong on screen — "the key is
//! blinking" and "this key wants its PIN" — and neither is something the core
//! knows about, because the core has no idea a cable exists. So they travel on
//! this channel instead of through `ShellOperation`.
//!
//! ## Why the PIN is not a core prompt
//!
//! `PromptKind` is the vocabulary of decisions the MACHINE branches on. A PIN
//! is not one: whether a particular authenticator has one, and how many
//! attempts it has left, are facts about a piece of hardware on one desk. A
//! shell that pushed them into the core would be asking four other clients —
//! three of which never see an authenticator directly — to carry a concept they
//! cannot have.
//!
//! ## Why the ceremony thread blocks
//!
//! Because it is holding the device open. The authenticator's PIN session is
//! per-connection: releasing it to go and ask a question would mean re-opening,
//! re-agreeing a shared secret, and spending one of a small number of PIN
//! attempts on a retry that was never refused. Blocking on a condvar until the
//! person answers is what keeps one attempt one attempt.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use crate::ctap::usb::TouchRequest;
use crate::executor::passkey::{Ceremony, CredentialChoice, PinRequest, WindowHandle};

/// The wallet picker's half of the channel. Same shape as the PIN's and for
/// the same reason: the ceremony thread is holding the device open while it
/// asks, so it blocks rather than releasing and re-enumerating.
#[derive(Default)]
struct PickState {
    asking: Option<Vec<CredentialChoice>>,
    /// The outer `Option` is "has an answer arrived", the inner is "a row, or a
    /// dismissal".
    answer: Option<Option<usize>>,
    closed: bool,
}

#[derive(Default)]
struct PinState {
    /// Set by the ceremony thread; taken by the screen.
    asking: Option<PinRequest>,
    /// Set by the screen; taken by the ceremony thread. The outer `Option` is
    /// "has an answer arrived", the inner is "was it a PIN or a dismissal".
    answer: Option<Option<String>>,
    /// The PIN already given during this flow, PER KEY.
    ///
    /// Keyed by device, and that is the whole point. One founding key takes
    /// several ceremonies — register, then prove, then confirm membership — and
    /// asking for the same digits three times in a row is not security, it is
    /// friction. But with two keys plugged in, the first version of this cache
    /// was flow-wide, so key A's PIN was silently offered to key B: wrong
    /// (different keys have different PINs, and it would spend one of B's
    /// attempts on A's), and it reads like the app kept the PIN somewhere.
    ///
    /// It keeps it in memory, for one flow, per device. Dropped the moment an
    /// attempt is REFUSED — so a mistyped PIN is never silently retried — and
    /// again when the flow closes.
    cached: HashMap<String, String>,
    /// The flow went away. A ceremony still blocked here must stop waiting
    /// rather than hold a thread until the process exits.
    closed: bool,
}

/// Shared between the screen and whatever ceremony is currently running.
#[derive(Default)]
pub struct CeremonyChannel {
    /// What the key is waiting for right now, if anything. A `Mutex` rather
    /// than an atomic flag because it carries the key's product string and
    /// which physical act is being asked for — a bool could only say "some
    /// key wants something".
    touch: Mutex<Option<TouchRequest>>,
    /// The caBLE QR payload the person scans with their phone, while a hybrid
    /// ceremony waits for the scan. `None` when no QR is up.
    qr: Mutex<Option<String>>,
    /// The person dismissed the QR (or the flow left). The hybrid scan polls
    /// this; a fresh QR going up clears it, so one dismissal cancels one
    /// ceremony and not the next.
    qr_cancelled: std::sync::atomic::AtomicBool,
    pin: Mutex<PinState>,
    pick: Mutex<PickState>,
    /// ONE CONDVAR PER MUTEX, and that is not a style choice.
    ///
    /// `std`'s `Condvar` remembers the mutex it was first waited on with and
    /// PANICS — aborting the process from a background thread — if a second
    /// one shows up. A single `answered` shared by the PIN wait and the wallet
    /// picker's wait is exactly that: "attempted to use a condition variable
    /// with two mutexes", raised the first time a person is asked for a PIN
    /// and then asked which wallet.
    pin_answered: Condvar,
    pick_answered: Condvar,
}

impl CeremonyChannel {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The executor-facing handle.
    ///
    /// `window` is the app's own window, which only the Windows path uses — the
    /// OS dialog parents itself to it. On macOS and Linux this shell draws its
    /// own prompts and the value is ignored.
    pub fn ceremony(self: &Arc<Self>, window: WindowHandle) -> Ceremony {
        let touch_channel = Arc::clone(self);
        let qr_channel = Arc::clone(self);
        let cancel_channel = Arc::clone(self);
        let pin_channel = Arc::clone(self);
        let pick_channel = Arc::clone(self);
        Ceremony {
            touch: Arc::new(move |waiting| {
                if let Ok(mut slot) = touch_channel.touch.lock() {
                    *slot = waiting;
                }
            }),
            qr: Arc::new(move |payload| {
                if payload.is_some() {
                    qr_channel
                        .qr_cancelled
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                }
                if let Ok(mut slot) = qr_channel.qr.lock() {
                    *slot = payload;
                }
            }),
            cancelled: Arc::new(move || {
                cancel_channel
                    .qr_cancelled
                    .load(std::sync::atomic::Ordering::SeqCst)
            }),
            pin: Arc::new(move |request| pin_channel.request_pin(request)),
            pick: Arc::new(move |choices| pick_channel.request_choice(choices)),
            window,
        }
    }

    /// What a key is waiting for right now, if anything.
    pub fn touch_waiting(&self) -> Option<TouchRequest> {
        self.touch.lock().ok()?.clone()
    }

    /// The caBLE QR to show right now, if a hybrid ceremony is waiting for a scan.
    pub fn qr_showing(&self) -> Option<String> {
        self.qr.lock().ok()?.clone()
    }

    /// The person dismissed the QR: take it down NOW and tell the scan to stop.
    ///
    /// Both halves matter. Hiding the card alone would leave a ninety-second
    /// Bluetooth scan running behind a screen that says nothing is happening,
    /// and the next attempt would start a second one beside it.
    pub fn cancel_qr(&self) {
        self.qr_cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut slot) = self.qr.lock() {
            *slot = None;
        }
    }

    /// Called on the ceremony thread. Blocks until the screen answers.
    fn request_pin(&self, request: PinRequest) -> Option<String> {
        let Ok(mut state) = self.pin.lock() else {
            return None;
        };
        if state.closed {
            return None;
        }
        // A refused attempt invalidates THAT KEY's entry: it was wrong, and
        // replaying it would spend the next attempt on the same mistake.
        let device = request.device.clone();
        if request.retry {
            state.cached.remove(&device);
        } else if let Some(cached) = state.cached.get(&device) {
            return Some(cached.clone());
        }

        state.asking = Some(request);
        state.answer = None;
        loop {
            if state.closed {
                return None;
            }
            if let Some(answer) = state.answer.take() {
                state.asking = None;
                match &answer {
                    Some(pin) => {
                        state.cached.insert(device, pin.clone());
                    }
                    None => {
                        state.cached.remove(&device);
                    }
                }
                return answer;
            }
            let Ok(next) = self.pin_answered.wait(state) else {
                return None;
            };
            state = next;
        }
    }

    /// Called on the ceremony thread. Blocks until the screen answers.
    fn request_choice(&self, choices: Vec<CredentialChoice>) -> Option<usize> {
        let Ok(mut state) = self.pick.lock() else {
            return None;
        };
        if state.closed {
            return None;
        }
        state.asking = Some(choices);
        state.answer = None;
        loop {
            if state.closed {
                return None;
            }
            if let Some(answer) = state.answer.take() {
                state.asking = None;
                return answer;
            }
            let Ok(next) = self.pick_answered.wait(state) else {
                return None;
            };
            state = next;
        }
    }

    /// Called on the UI thread each tick: is a wallet being asked for?
    pub fn pending_choice(&self) -> Option<Vec<CredentialChoice>> {
        self.pick.lock().ok()?.asking.clone()
    }

    /// Called on the UI thread. `None` is a dismissal.
    pub fn answer_choice(&self, index: Option<usize>) {
        if let Ok(mut state) = self.pick.lock() {
            state.answer = Some(index);
        }
        self.pick_answered.notify_all();
    }

    /// Called on the UI thread each tick: is a PIN being asked for?
    pub fn pending_pin(&self) -> Option<PinRequest> {
        self.pin.lock().ok()?.asking.clone()
    }

    /// Called on the UI thread. `None` is a dismissal, which the ceremony reads
    /// as a cancellation.
    pub fn answer_pin(&self, value: Option<String>) {
        if let Ok(mut state) = self.pin.lock() {
            state.answer = Some(value);
        }
        self.pin_answered.notify_all();
    }

    /// The flow is leaving. Releases anything still blocked, and forgets the
    /// PIN — a cached secret outliving the screen that collected it is a
    /// secret nobody agreed to leave lying around.
    pub fn close(&self) {
        if let Ok(mut state) = self.pin.lock() {
            state.closed = true;
            state.cached.clear();
            state.asking = None;
            state.answer = Some(None);
        }
        if let Ok(mut state) = self.pick.lock() {
            state.closed = true;
            state.asking = None;
            state.answer = Some(None);
        }
        if let Ok(mut slot) = self.touch.lock() {
            *slot = None;
        }
        if let Ok(mut slot) = self.qr.lock() {
            *slot = None;
        }
        // A scan still waiting for a phone must not outlive the flow by up to
        // ninety seconds.
        self.qr_cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.pin_answered.notify_all();
        self.pick_answered.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dismissing the QR is two things at once, and a screen that did only one
    /// of them is the bug: the card comes down AND the scan is told to stop.
    /// One dismissal cancels one ceremony — the next QR starts clean — and a
    /// flow that leaves stops a scan still waiting for a phone.
    #[test]
    fn dismissing_the_qr_hides_it_and_stops_the_scan_once() {
        let channel = CeremonyChannel::new();
        let ceremony = channel.ceremony(0);

        (ceremony.qr)(Some("FIDO:/1".to_owned()));
        assert_eq!(channel.qr_showing().as_deref(), Some("FIDO:/1"));
        assert!(
            !(ceremony.cancelled)(),
            "a QR just shown is not a dismissed one"
        );

        channel.cancel_qr();
        assert_eq!(channel.qr_showing(), None, "the card comes down at once");
        assert!((ceremony.cancelled)(), "and the scan behind it is told");

        // The next attempt: the dismissal belonged to the last one.
        (ceremony.qr)(Some("FIDO:/2".to_owned()));
        assert!(!(ceremony.cancelled)());

        // The ceremony taking its own QR down is not a dismissal.
        (ceremony.qr)(None);
        assert!(!(ceremony.cancelled)());

        (ceremony.qr)(Some("FIDO:/3".to_owned()));
        channel.close();
        assert!(
            (ceremony.cancelled)(),
            "a flow that leaves stops the scan too"
        );
    }
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn request(device: &str, retry: bool) -> PinRequest {
        PinRequest {
            product: "YubiKey 5C NFC".to_owned(),
            device: device.to_owned(),
            retries: Some(8),
            retry,
        }
    }

    /// Answer whatever the ceremony thread asks for, counting the asks.
    ///
    /// The channel blocks the caller until a screen replies, so the "screen"
    /// here is a thread that watches for a question and answers it.
    fn answering(
        channel: &Arc<CeremonyChannel>,
        pin: &'static str,
        asks: Arc<AtomicUsize>,
    ) -> std::thread::JoinHandle<()> {
        let channel = Arc::clone(channel);
        std::thread::spawn(move || {
            // Bounded so a broken cache cannot hang the suite.
            for _ in 0..200 {
                if channel.pending_pin().is_some() {
                    asks.fetch_add(1, Ordering::Relaxed);
                    channel.answer_pin(Some(pin.to_owned()));
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        })
    }

    /// TWO KEYS, TWO PINS.
    ///
    /// The cache exists so one key is not asked three times during one wallet
    /// creation. It was flow-wide at first, which meant the second key plugged
    /// in never got asked at all — it was handed the first key's PIN. That is
    /// wrong twice over: different keys have different PINs, so it would spend
    /// one of the second key's limited attempts on the first key's digits; and
    /// to a person watching, it looks like the app kept their PIN.
    #[test]
    fn one_key_s_pin_is_never_offered_to_another() {
        let channel = CeremonyChannel::new();
        let asks = Arc::new(AtomicUsize::new(0));
        let screen = answering(&channel, "1234", Arc::clone(&asks));

        let ceremony = channel.ceremony(0);
        // Same key twice: asked once, cached for the second.
        assert_eq!(
            (ceremony.pin)(request("/dev/key-a", false)).as_deref(),
            Some("1234")
        );
        assert_eq!(
            (ceremony.pin)(request("/dev/key-a", false)).as_deref(),
            Some("1234")
        );
        assert_eq!(
            asks.load(Ordering::Relaxed),
            1,
            "the same key is asked once"
        );

        // A DIFFERENT key: asked again, even though a PIN is already cached.
        assert_eq!(
            (ceremony.pin)(request("/dev/key-b", false)).as_deref(),
            Some("1234")
        );
        assert_eq!(
            asks.load(Ordering::Relaxed),
            2,
            "a second key must be asked for its own PIN"
        );

        channel.close();
        let _ = screen.join();
    }

    /// A refused PIN drops that key's entry, and only that key's.
    #[test]
    fn a_refusal_forgets_one_key_and_leaves_the_other() {
        let channel = CeremonyChannel::new();
        let asks = Arc::new(AtomicUsize::new(0));
        let screen = answering(&channel, "1234", Arc::clone(&asks));

        let ceremony = channel.ceremony(0);
        let _ = (ceremony.pin)(request("/dev/key-a", false));
        let _ = (ceremony.pin)(request("/dev/key-b", false));
        assert_eq!(asks.load(Ordering::Relaxed), 2);

        // A retries: asked again. B is untouched by that.
        let _ = (ceremony.pin)(request("/dev/key-a", true));
        assert_eq!(asks.load(Ordering::Relaxed), 3);
        let _ = (ceremony.pin)(request("/dev/key-b", false));
        assert_eq!(
            asks.load(Ordering::Relaxed),
            3,
            "the other key's cache survives its neighbour's mistake"
        );

        channel.close();
        let _ = screen.join();
    }

    /// BOTH waits, on one channel.
    ///
    /// `std`'s `Condvar` remembers the mutex it was first waited on with and
    /// panics if a second one appears — and a panic on a ceremony thread
    /// aborts the process, which is what a person saw the first time they were
    /// asked for a PIN and then which wallet. One condvar was serving two
    /// mutexes.
    ///
    /// The test is the SEQUENCE, not either half: each worked perfectly alone,
    /// which is why the suite that covered them separately said nothing.
    #[test]
    fn a_pin_and_a_wallet_choice_can_both_be_asked_on_one_channel() {
        let channel = CeremonyChannel::new();
        let ceremony = channel.ceremony(0);

        let screen = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || {
                for _ in 0..400 {
                    if channel.pending_pin().is_some() {
                        channel.answer_pin(Some("1234".to_owned()));
                    }
                    if channel.pending_choice().is_some() {
                        channel.answer_choice(Some(1));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })
        };

        assert_eq!(
            (ceremony.pin)(request("/dev/key-a", false)).as_deref(),
            Some("1234")
        );
        assert_eq!(
            (ceremony.pick)(vec![choice("Everyday wallet"), choice("Savings")]),
            Some(1)
        );
        // And back the other way, because the panic is about which mutex was
        // FIRST — a channel that survives pin-then-pick could still die on
        // pick-then-pin if only one of the two were split.
        assert_eq!(
            (ceremony.pin)(request("/dev/key-b", false)).as_deref(),
            Some("1234")
        );

        channel.close();
        let _ = screen.join();
    }

    fn choice(name: &str) -> CredentialChoice {
        CredentialChoice {
            name: name.to_owned(),
            credential_id: "aabbccdd".to_owned(),
            product: "YubiKey 5C NFC".to_owned(),
        }
    }

    /// Closing the flow releases anything blocked AND forgets every PIN. A
    /// cached secret outliving the screen that collected it is a secret nobody
    /// agreed to leave lying around.
    #[test]
    fn closing_forgets_every_pin_and_unblocks() {
        let channel = CeremonyChannel::new();
        let asks = Arc::new(AtomicUsize::new(0));
        let screen = answering(&channel, "1234", Arc::clone(&asks));
        let ceremony = channel.ceremony(0);
        let _ = (ceremony.pin)(request("/dev/key-a", false));
        channel.close();
        let _ = screen.join();

        // After close, a request returns immediately with nothing rather than
        // blocking on a screen that is gone.
        assert_eq!((ceremony.pin)(request("/dev/key-a", false)), None);
    }
}
