//! The wait before the passkey, bounded and said (r5reg-3) — DESKTOP.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/core/prepare-bound.ts`.
//!
//! A send reads several things from the network before its passkey prompt
//! can open — the account's nonce, whether it is deployed, the gas, the fee.
//! With a network's nodes answering 429 those reads held the confirm on a
//! spinner with nothing said, and what ended it was worded as a generic
//! failure although no prompt had opened yet.
//!
//! A [`Gate`] stands at the moment the passkey opens. Until it opens a
//! [`Watch`] reports, past [`PREPARE_SLOW`] and every [`TICK`] after, how long
//! it has been and whether the network's nodes say they are busy
//! (`send::Event::SubmitSlow`); past [`PREPARE_LIMIT`] the gate shuts for
//! good — a passkey never opens after the wait was given up, so nothing can
//! be signed behind the person's back. Once it has opened, the bound is over:
//! the ceremony has its own deadline.
//!
//! One difference from the web, said plainly: the web answers the submit the
//! instant the limit passes. Here the submit is one blocking call, so the
//! answer comes when its read returns — with the gate shut, it can only come
//! back unsigned, and it is told as the network's ([`classify`]).

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use vela_core::app::send::SendSubmitFailure;

use crate::executor::user_op::SubmitFailure;

/// How long the reads may take before the confirm says what it is waiting for.
pub const PREPARE_SLOW: Duration = Duration::from_millis(5_000);
/// How long the reads may take before the submit gives up — nothing signed.
pub const PREPARE_LIMIT: Duration = Duration::from_millis(45_000);
/// How often the time said is brought up to date.
pub const TICK: Duration = Duration::from_millis(1_000);

#[derive(Default)]
struct State {
    opened: bool,
    given_up: bool,
    /// The submit returned: the watch stops.
    done: bool,
}

/// The moment the passkey opens, and whether the wait before it was given up.
#[derive(Default)]
pub struct Gate {
    state: Mutex<State>,
    wake: Condvar,
}

impl Gate {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Right before the passkey opens: `true` — open it — unless the wait was
    /// given up, which shuts the gate for good. Either way the watch stops
    /// reporting.
    pub fn open(&self) -> bool {
        let mut state = self.lock();
        if state.given_up {
            return false;
        }
        state.opened = true;
        self.wake.notify_all();
        true
    }

    #[must_use]
    pub fn opened(&self) -> bool {
        self.lock().opened
    }

    #[must_use]
    pub fn given_up(&self) -> bool {
        self.lock().given_up
    }

    /// The limit passed with the gate still shut: it stays shut.
    fn give_up(&self) -> bool {
        let mut state = self.lock();
        if state.opened || state.done {
            return false;
        }
        state.given_up = true;
        true
    }

    fn finish(&self) {
        let mut state = self.lock();
        state.done = true;
        self.wake.notify_all();
    }

    /// Wait up to `wait`, or until the gate opened or the submit returned.
    /// `true` when the watch should stop.
    fn sleep(&self, wait: Duration) -> bool {
        let state = self.lock();
        if state.opened || state.done {
            return true;
        }
        let (state, _) = self
            .wake
            .wait_timeout(state, wait)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.opened || state.done
    }
}

/// The reporting thread, stopped when the submit returns.
pub struct Watch {
    gate: Arc<Gate>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watch {
    /// The submit returned: stop reporting.
    pub fn finish(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.gate.finish();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Watch `gate` from now: `on_slow(elapsed_ms, busy)` every [`TICK`] past
/// [`PREPARE_SLOW`] while it is shut, and the gate given up at
/// [`PREPARE_LIMIT`].
pub fn watch(
    gate: Arc<Gate>,
    busy: impl Fn() -> bool + Send + 'static,
    on_slow: impl Fn(u32, bool) + Send + 'static,
) -> Watch {
    watch_with(gate, PREPARE_SLOW, TICK, PREPARE_LIMIT, busy, on_slow)
}

fn watch_with(
    gate: Arc<Gate>,
    slow: Duration,
    tick: Duration,
    limit: Duration,
    busy: impl Fn() -> bool + Send + 'static,
    on_slow: impl Fn(u32, bool) + Send + 'static,
) -> Watch {
    let started = Instant::now();
    let watched = Arc::clone(&gate);
    let thread = std::thread::spawn(move || {
        let mut wait = slow;
        loop {
            if watched.sleep(wait) {
                return;
            }
            let elapsed = started.elapsed();
            if elapsed >= limit {
                watched.give_up();
                return;
            }
            if elapsed >= slow {
                on_slow(
                    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX),
                    busy(),
                );
            }
            wait = tick.min(limit.saturating_sub(elapsed));
        }
    });
    Watch {
        gate,
        thread: Some(thread),
    }
}

/// The submit's failure in the core's words, knowing which side of the gate
/// it came from (the web's `BeforePasskeyError` + `networkFailed`): a wait
/// given up, or a read before the passkey that failed while the network's
/// nodes were failing or busy, is the network's — `ChainUnreachable`, never
/// the passkey's. Anything else, and anything after the passkey opened, is
/// classified as it always was.
#[must_use]
pub fn classify(
    failure: SubmitFailure,
    gate: &Gate,
    network_failing: bool,
    busy: bool,
) -> SendSubmitFailure {
    let before_passkey = !gate.opened();
    let network = gate.given_up()
        || (before_passkey && network_failing && matches!(failure, SubmitFailure::Other(_)));
    if network {
        return SendSubmitFailure::ChainUnreachable { busy };
    }
    failure.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Past the slow mark the wait is said, every tick, with the time it
    /// has taken; an opened gate stops it.
    #[test]
    fn the_wait_is_said_until_the_passkey_opens() {
        let gate = Arc::new(Gate::default());
        let said = Arc::new(AtomicU32::new(0));
        let counted = Arc::clone(&said);
        let watch = watch_with(
            Arc::clone(&gate),
            Duration::from_millis(20),
            Duration::from_millis(10),
            Duration::from_secs(60),
            || true,
            move |elapsed_ms, busy| {
                assert!(elapsed_ms >= 20 && busy);
                counted.fetch_add(1, Ordering::SeqCst);
            },
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(gate.open(), "not given up: the passkey opens");
        let at_open = said.load(Ordering::SeqCst);
        assert!(at_open >= 2, "said each tick past the mark ({at_open})");
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(said.load(Ordering::SeqCst), at_open, "nothing after it opened");
        watch.finish();
        assert!(!gate.given_up());
    }

    /// Past the limit the gate shuts for good: no passkey opens after the
    /// wait was given up, and the failure is the network's.
    #[test]
    fn a_wait_given_up_never_opens_the_passkey() {
        let gate = Arc::new(Gate::default());
        let watch = watch_with(
            Arc::clone(&gate),
            Duration::from_millis(5),
            Duration::from_millis(5),
            Duration::from_millis(30),
            || false,
            |_, _| {},
        );
        std::thread::sleep(Duration::from_millis(90));
        assert!(gate.given_up());
        assert!(!gate.open(), "shut for good");
        watch.finish();
        assert_eq!(
            classify(SubmitFailure::PasskeyCancelled, &gate, false, true),
            SendSubmitFailure::ChainUnreachable { busy: true }
        );
    }

    /// A submit that returns in time stops the watch: nothing is said.
    #[test]
    fn a_quick_submit_says_nothing() {
        let gate = Arc::new(Gate::default());
        let said = Arc::new(AtomicU32::new(0));
        let counted = Arc::clone(&said);
        let watch = watch_with(
            Arc::clone(&gate),
            Duration::from_millis(200),
            Duration::from_millis(10),
            Duration::from_secs(5),
            || false,
            move |_, _| {
                counted.fetch_add(1, Ordering::SeqCst);
            },
        );
        watch.finish();
        assert_eq!(said.load(Ordering::SeqCst), 0);
        assert!(!gate.given_up());
    }

    /// A read that failed before the passkey while the network was failing
    /// is the network's; the same words after the passkey opened, or with
    /// the network answering, keep their old reading — and a passkey closed
    /// is the person's, never the network's.
    #[test]
    fn only_a_failure_before_the_passkey_on_a_failing_network_is_the_networks() {
        let shut = Gate::default();
        assert_eq!(
            classify(SubmitFailure::Other("timeout".into()), &shut, true, false),
            SendSubmitFailure::ChainUnreachable { busy: false }
        );
        assert_eq!(
            classify(SubmitFailure::Other("bad".into()), &shut, false, false),
            SendSubmitFailure::Other {
                message: Some("bad".into())
            }
        );
        assert_eq!(
            classify(SubmitFailure::PasskeyCancelled, &shut, true, false),
            SendSubmitFailure::PasskeyCancelled
        );
        let opened = Gate::default();
        assert!(opened.open());
        assert_eq!(
            classify(SubmitFailure::Other("timeout".into()), &opened, true, true),
            SendSubmitFailure::Other {
                message: Some("timeout".into())
            }
        );
    }
}
