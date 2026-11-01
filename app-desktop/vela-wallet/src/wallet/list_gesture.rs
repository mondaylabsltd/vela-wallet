//! The home's and the history's columns scrolling, as the incoming check's
//! cadence hears it (r10 follow-up, docs/multisig-machines.md §9.4 / §10):
//! `interacting {on}` once a column starts moving under the wheel or the
//! trackpad, `interacting {off}` once it has been still for [`SETTLE`]. The
//! core holds a scan that comes due meanwhile and runs it at `off`; when to
//! scan stays the core's.
//!
//! A column's own state ([`crate::ui::SmoothScroll::moving`]) flickers
//! between a trackpad's reports — a gesture is caught up with between two of
//! them — so a gesture ends only once nothing has moved for a while: the
//! desktop's `scrollend`. Transport only: each edge is reported once, and an
//! `on` is always followed by its `off`.

use std::time::{Duration, Instant};

/// How long the columns must be still for a gesture to have ended: longer
/// than the gaps between a trackpad's reports and a wheel's notches, short
/// enough that a held scan runs right after.
pub const SETTLE: Duration = Duration::from_millis(250);

/// How often a gesture in progress is looked at — no frame is drawn once the
/// columns stop, so the end is noticed by a timer, not by `render`.
pub const LOOK: Duration = Duration::from_millis(50);

/// One gesture's edges, from looks at whether the columns are moving.
#[derive(Debug, Default)]
pub struct ListGesture {
    on: bool,
    still_since: Option<Instant>,
}

impl ListGesture {
    /// A gesture is in progress (an `on` was reported, its `off` not yet).
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// One look: `Some(true)` when a gesture has just started, `Some(false)`
    /// when it has just ended, `None` when nothing changed.
    pub fn look(&mut self, moving: bool, now: Instant) -> Option<bool> {
        if moving {
            self.still_since = None;
            if self.on {
                return None;
            }
            self.on = true;
            return Some(true);
        }
        if !self.on {
            return None;
        }
        let since = *self.still_since.get_or_insert(now);
        if now.duration_since(since) < SETTLE {
            return None;
        }
        self.on = false;
        self.still_since = None;
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scroll is one `on` and one `off`, however many frames it moves.
    #[test]
    fn a_scroll_is_one_on_and_one_off() {
        let mut gesture = ListGesture::default();
        let t0 = Instant::now();
        let mut edges = Vec::new();
        for frame in 0..60u64 {
            edges.extend(gesture.look(true, t0 + Duration::from_millis(frame * 8)));
        }
        assert_eq!(edges, [true]);
        let stopped = t0 + Duration::from_millis(480);
        assert_eq!(gesture.look(false, stopped), None);
        assert_eq!(gesture.look(false, stopped + LOOK), None);
        assert!(gesture.is_on());
        assert_eq!(gesture.look(false, stopped + SETTLE), Some(false));
        assert!(!gesture.is_on());
        assert_eq!(gesture.look(false, stopped + SETTLE * 4), None);
    }

    /// A trackpad's gesture is caught up with between two of its reports:
    /// a still frame or two inside it does not end it.
    #[test]
    fn a_pause_shorter_than_the_settle_is_the_same_gesture() {
        let mut gesture = ListGesture::default();
        let t0 = Instant::now();
        assert_eq!(gesture.look(true, t0), Some(true));
        assert_eq!(gesture.look(false, t0 + LOOK), None);
        assert_eq!(gesture.look(false, t0 + LOOK * 3), None);
        assert_eq!(gesture.look(true, t0 + LOOK * 4), None);
        // Still again: the settle counts from here, not from the first pause.
        let still = t0 + LOOK * 5;
        assert_eq!(gesture.look(false, still), None);
        assert_eq!(gesture.look(false, still + SETTLE - LOOK), None);
        assert_eq!(gesture.look(false, still + SETTLE), Some(false));
    }

    /// Still columns say nothing.
    #[test]
    fn nothing_moving_reports_nothing() {
        let mut gesture = ListGesture::default();
        let t0 = Instant::now();
        assert_eq!(gesture.look(false, t0), None);
        assert_eq!(gesture.look(false, t0 + SETTLE * 2), None);
    }
}
