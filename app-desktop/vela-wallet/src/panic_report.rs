//! A panic becomes a sheet, not a vanished window (spec 038 finding 13).
//!
//! Ten places spawn work off the main thread and none of them caught a
//! panic: a USB key yanked mid-ceremony or a malformed RPC body hitting the
//! wrong `unwrap` on a background thread was the whole app gone, with no
//! sheet and no text a person could send us. Two pieces fix that:
//!
//! - [`guarded`] wraps the work the resident pump hands to gpui's background
//!   executor. A panic inside it is caught, logged, and the machine's
//!   operation is simply not resolved — the screen keeps its last state and
//!   the person can go Back — instead of unwinding into the executor.
//! - [`install`] sets the process panic hook, which writes the message and
//!   location into a mailbox the pages read on their next render and raise
//!   as the ordinary failure sheet (title "Something went wrong", the
//!   details expandable, the report button copying them).
//!
//! The hook runs for EVERY panic, including one on the main thread that will
//! still abort the process; the mailbox is for the ones [`guarded`] survives.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

fn mailbox() -> &'static Mutex<Option<String>> {
    static REPORT: Mutex<Option<String>> = Mutex::new(None);
    &REPORT
}

/// Install the process hook. Idempotent in effect: calling it twice replaces
/// one identical hook with another.
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic with a non-string payload".to_owned());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_owned());
        let report = format!("{message}\n    at {location}");
        eprintln!("[vela-wallet] panic: {report}");
        if let Ok(mut slot) = mailbox().lock() {
            *slot = Some(report);
        }
    }));
}

/// Run `work`, surviving a panic inside it. `None` when it panicked.
pub fn guarded<T>(work: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(work)).ok()
}

/// Take the pending report, if a survived panic left one. The page that
/// takes it owns showing it.
pub fn take() -> Option<String> {
    mailbox().lock().ok().and_then(|mut slot| slot.take())
}

/// Dev seam (quickstart SC-432): `VELA_TEST_PANIC=1` makes the next guarded
/// work panic, once, so the sheet can be seen without breaking anything real.
pub fn test_panic_requested() -> bool {
    static FIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    std::env::var("VELA_TEST_PANIC").as_deref() == Ok("1")
        && !FIRED.swap(true, std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SC-430/432: a panic inside guarded work is survived and reported; the
    /// caller gets `None` and the mailbox gets the words.
    #[test]
    fn a_panic_is_survived_and_reported() {
        install();
        let _ = take();
        let result: Option<u32> = guarded(|| panic!("the key was unplugged"));
        assert_eq!(result, None);
        let report = take().expect("the hook left a report");
        assert!(report.contains("the key was unplugged"), "{report}");
        assert!(report.contains("panic_report.rs"), "location recorded: {report}");
        assert_eq!(take(), None, "taken once");
        assert_eq!(guarded(|| 7), Some(7), "and ordinary work is untouched");
    }
}
