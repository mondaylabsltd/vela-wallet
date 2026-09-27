//! A column that scrolls the way a browser scrolls: once per frame, eased.
//!
//! gpui moves a scrolling column the moment a wheel event arrives, by exactly
//! that event's delta. Neither kind of input arrives in step with the display.
//! A trackpad reports on its own clock, so at 120 Hz one frame carries two
//! reports and the next carries none, and the page lurches at a perfectly
//! steady frame rate. A mouse notch is a whole step in a single frame. A
//! browser does neither — it spreads a trackpad's reports evenly across frames
//! and glides a notch — and that is the smoothness the web had and the desktop
//! did not.
//!
//! So a column keeps gpui's clipping and its `ScrollHandle` — the scrollbar,
//! the resets to the top and the clamp to the content all still read and write
//! it — but not gpui's wheel handling. `overflow_y_hidden` turns that off (gpui
//! only moves an axis whose overflow is `Scroll`); a wheel event moves a
//! TARGET; and every frame puts the column where the input says it should be
//! at that frame's moment. For a trackpad that is its reports resampled onto
//! the frame's clock, a few milliseconds behind the newest, so every frame
//! lands between two reports — what Chrome does. For a notch it is a glide
//! that closes on the target over about a tenth of a second.
//!
//! **The glide advances only when `step` runs**, once per frame, before the
//! column is laid out. `attach` does both for a builder that has the window;
//! a view whose columns are built without it steps them all at the top of its
//! `render` and uses `wire`. A column wired and never stepped takes the wheel
//! and does not move.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    EntityId, ScrollDelta, ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement, Styled,
    Window, point, px,
};

/// How far behind the newest trackpad report a frame is drawn: longer than
/// the usual gap between two reports, so a frame nearly always falls between
/// two of them, and short enough that the page stays under the fingers.
const RESAMPLE_DELAY: Duration = Duration::from_millis(10);
/// Where a gesture starts: the page as it stands, this long before the first
/// report, so its first frames ramp up rather than jump.
const REPORT_GAP: Duration = Duration::from_millis(8);
/// A pause this long between two reports is a new gesture, not a slow one:
/// it starts again from where the page stands.
const NEW_GESTURE: Duration = Duration::from_millis(50);
/// Reports older than the frame being drawn are dropped; this many at most
/// are ever kept.
const MAX_REPORTS: usize = 32;
/// A notch's glide, as a critically damped spring: eased in and out, most of
/// the way in about 100 ms, and — because the page's speed carries from one
/// notch into the next — an even run when the wheel keeps turning, rather
/// than a lurch per notch.
const WHEEL_OMEGA: f32 = 30.;
/// One notch, as a browser on the Mac counts it (Chromium's
/// `kScrollbarPixelsPerCocoaTick`).
const PX_PER_LINE: f32 = 40.;
/// Closer than this, and slower than that, and the glide is over.
const SETTLE_PX: f32 = 0.25;
const SETTLE_SPEED: f32 = 8.;
/// A frame longer than this is a stall, not motion to catch up on in one go.
const MAX_STEP_MS: f32 = 50.;

/// A column's scroll position and its glide. Clone it into the element each
/// frame; the clones share one position.
#[derive(Clone, Default)]
pub struct SmoothScroll {
    handle: ScrollHandle,
    motion: Rc<RefCell<Motion>>,
}

#[derive(Default)]
struct Motion {
    /// Where the input has taken the column: px down from its top.
    target: f32,
    /// Where the glide is, unrounded.
    shown: f32,
    /// What was last written to the handle — `shown` on the device-pixel grid.
    drawn: f32,
    /// The glide's speed, px/s down, carried across notches.
    velocity: f32,
    /// A trackpad gesture's reports, oldest first: when each arrived and where
    /// it put the target. Empty while a notch glides, or nothing moves.
    reports: VecDeque<(Instant, f32)>,
    moving: bool,
    last_frame: Option<Instant>,
}

/// The handle is the column's position; anything that only reads it, or sets
/// it outright (a reset to the top), uses it as before.
impl std::ops::Deref for SmoothScroll {
    type Target = ScrollHandle;

    fn deref(&self) -> &ScrollHandle {
        &self.handle
    }
}

impl SmoothScroll {
    pub fn new() -> Self {
        Self::default()
    }

    /// This frame's step, then [`wire`](Self::wire) for the view being
    /// rendered. For a column built where the window is at hand.
    pub fn attach<E>(&self, element: E, window: &Window) -> E
    where
        E: StatefulInteractiveElement + Styled,
    {
        self.step(window);
        self.wire(element, window.current_view())
    }

    /// Make `element` the column: it clips vertically, it is positioned by
    /// this scroll, and the wheel over it moves this scroll and redraws
    /// `view`. The column only moves if something calls [`step`](Self::step)
    /// every frame.
    pub fn wire<E>(&self, element: E, view: EntityId) -> E
    where
        E: StatefulInteractiveElement + Styled,
    {
        let this = self.clone();
        element
            .track_scroll(&self.handle)
            .overflow_y_hidden()
            .on_scroll_wheel(move |event, _, cx| {
                if this.push(event) {
                    cx.notify(view);
                    // Handled here, so a column around this one does not
                    // scroll too. At an end it is not handled, and goes on.
                    cx.stop_propagation();
                }
            })
    }

    /// Move the target by one wheel event. False when the column cannot move
    /// that way — at an end, or with nothing to scroll — so the event can go
    /// on to whatever is around it.
    fn push(&self, event: &ScrollWheelEvent) -> bool {
        let (delta, trackpad) = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.y), true),
            ScrollDelta::Lines(delta) => (delta.y * PX_PER_LINE, false),
        };
        if delta == 0. {
            return false;
        }
        let max = f32::from(self.handle.max_offset().y).max(0.);
        let mut motion = self.motion.borrow_mut();
        let Some(target) = next_target(motion.target, delta, max) else {
            return false;
        };
        motion.target = target;
        if trackpad {
            let now = Instant::now();
            let fresh = motion
                .reports
                .back()
                .is_none_or(|(time, _)| now.duration_since(*time) > NEW_GESTURE);
            if fresh {
                let from = motion.shown;
                motion.reports.clear();
                let start = now.checked_sub(REPORT_GAP).unwrap_or(now);
                motion.reports.push_back((start, from));
            }
            motion.reports.push_back((now, target));
            motion.velocity = 0.;
            while motion.reports.len() > MAX_REPORTS {
                motion.reports.pop_front();
            }
        } else {
            motion.reports.clear();
        }
        if !motion.moving {
            motion.moving = true;
            motion.last_frame = None;
        }
        true
    }

    /// This frame's step: put the column where the input says it is at this
    /// moment, write that to the handle, and ask for the next frame until the
    /// input has been caught up with. Once per frame, before the column is
    /// laid out.
    pub fn step(&self, window: &Window) {
        let mut motion = self.motion.borrow_mut();
        let on_handle = -f32::from(self.handle.offset().y);
        // Something else put the column somewhere — a reset to the top, or
        // gpui's clamp after the content got shorter. That wins, and the glide
        // starts again from there.
        if (on_handle - motion.drawn).abs() > 0.01 {
            *motion = Motion {
                target: on_handle,
                shown: on_handle,
                drawn: on_handle,
                ..Motion::default()
            };
            return;
        }
        if !motion.moving {
            return;
        }
        let now = Instant::now();
        // The first step of a glide has no frame before it: call it one frame.
        let step_ms = motion.last_frame.map_or(1000. / 120., |last| {
            (now.duration_since(last).as_secs_f32() * 1000.).min(MAX_STEP_MS)
        });
        motion.last_frame = Some(now);
        let max = f32::from(self.handle.max_offset().y).max(0.);
        motion.target = motion.target.clamp(0., max);
        let caught_up = if motion.reports.is_empty() {
            let (shown, velocity) = spring(
                motion.shown,
                motion.velocity,
                motion.target,
                step_ms / 1000.,
            );
            motion.shown = shown.clamp(0., max);
            motion.velocity = velocity;
            motion.velocity.abs() < SETTLE_SPEED
        } else {
            let at = now.checked_sub(RESAMPLE_DELAY).unwrap_or(now);
            motion.shown = resample(&motion.reports, at).clamp(0., max);
            // Only the report at or before this frame's moment, and those
            // after it, can matter to a later frame.
            while motion.reports.get(1).is_some_and(|(time, _)| *time <= at) {
                motion.reports.pop_front();
            }
            // Past the newest report the page holds there. A report that is
            // merely late continues from it; a gesture that has ended stops
            // asking for frames.
            motion.reports.back().is_some_and(|(time, _)| *time <= at)
        };
        if caught_up && (motion.target - motion.shown).abs() < SETTLE_PX {
            motion.shown = motion.target;
            motion.velocity = 0.;
            motion.moving = false;
            motion.last_frame = None;
        }
        // On the device-pixel grid, so text does not shimmer between two
        // renderings of the same glyph as it slides.
        let scale = window.scale_factor();
        motion.drawn = (motion.shown * scale).round() / scale;
        let x = self.handle.offset().x;
        self.handle.set_offset(point(x, px(-motion.drawn)));
        if motion.moving {
            window.request_animation_frame();
        }
    }
}

/// Where one wheel event takes the target, or `None` when it cannot move.
/// gpui's delta is positive when the content should come down — when the
/// column scrolls back towards its top.
fn next_target(target: f32, delta: f32, max: f32) -> Option<f32> {
    let next = (target - delta).clamp(0., max);
    ((next - target).abs() >= f32::EPSILON).then_some(next)
}

/// Where the reports put the column at `at`: between the two around it, in
/// proportion; before the first, the first; after the last, the last.
fn resample(reports: &VecDeque<(Instant, f32)>, at: Instant) -> f32 {
    let mut before: Option<(Instant, f32)> = None;
    for &(time, value) in reports {
        if time > at {
            let Some((from_time, from)) = before else {
                return value;
            };
            let span = time.duration_since(from_time).as_secs_f32();
            if span <= 0. {
                return value;
            }
            let t = at.duration_since(from_time).as_secs_f32() / span;
            return from + (value - from) * t;
        }
        before = Some((time, value));
    }
    before.map_or(0., |(_, value)| value)
}

/// One frame's step of a critically damped spring pulling `shown` to
/// `target`, solved exactly rather than integrated, so a long frame cannot
/// make it overshoot or ring. Returns the new position and speed.
fn spring(shown: f32, velocity: f32, target: f32, step_s: f32) -> (f32, f32) {
    let gap = shown - target;
    let lead = velocity + WHEEL_OMEGA * gap;
    let decay = (-WHEEL_OMEGA * step_s).exp();
    (
        target + (gap + lead * step_s) * decay,
        (velocity - WHEEL_OMEGA * lead * step_s) * decay,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At an end, or with nothing to scroll, the event is left for whatever is
    /// around the column.
    #[test]
    fn a_column_that_cannot_move_passes_the_wheel_on() {
        assert_eq!(next_target(0., 40., 0.), None, "nothing to scroll");
        assert_eq!(next_target(0., 40., 500.), None, "already at the top");
        assert_eq!(next_target(500., -40., 500.), None, "already at the bottom");
        assert_eq!(next_target(0., -40., 500.), Some(40.));
        assert_eq!(
            next_target(480., -40., 500.),
            Some(500.),
            "stops at the end"
        );
    }

    /// A notch glides — a little on its first frame, most of the way within
    /// about a tenth of a second, all of it soon after — and never overshoots.
    #[test]
    fn a_notch_glides_and_settles_without_overshooting() {
        let frame = 1. / 120.;
        let (mut shown, mut velocity) = (0f32, 0f32);
        let mut frames = 0;
        while (PX_PER_LINE - shown).abs() >= SETTLE_PX || velocity.abs() >= SETTLE_SPEED {
            (shown, velocity) = spring(shown, velocity, PX_PER_LINE, frame);
            assert!(shown <= PX_PER_LINE, "overshot to {shown}");
            frames += 1;
            if frames == 1 {
                assert!(
                    shown > 0. && shown < PX_PER_LINE / 4.,
                    "{shown} on the first frame"
                );
            }
            if frames == 15 {
                assert!(shown > PX_PER_LINE * 0.75, "{shown} after 125 ms");
            }
        }
        assert!(frames < 45, "took {frames} frames");
    }

    /// Notch after notch, the page keeps its speed instead of stopping and
    /// starting again for each one.
    #[test]
    fn a_turning_wheel_scrolls_at_an_even_pace() {
        let frame = 1. / 120.;
        let (mut shown, mut velocity, mut target) = (0f32, 0f32, 0f32);
        let mut moves = Vec::new();
        for n in 0..96 {
            // A notch every 8 frames — a wheel turned at 15 notches a second.
            if n % 8 == 0 {
                target += PX_PER_LINE;
            }
            let before = shown;
            (shown, velocity) = spring(shown, velocity, target, frame);
            if n >= 48 {
                moves.push(shown - before);
            }
        }
        let fastest = moves.iter().copied().fold(f32::MIN, f32::max);
        let slowest = moves.iter().copied().fold(f32::MAX, f32::min);
        assert!(slowest > 0., "stood still between notches");
        assert!(fastest / slowest < 1.6, "{slowest}..{fastest} px a frame");
    }

    /// The glide depends on time, not on how many frames it was cut into: a
    /// 60 Hz display and a 120 Hz one show the page at the same place at the
    /// same moment.
    #[test]
    fn the_glide_is_the_same_at_any_frame_rate() {
        let (one, _) = spring(0., 0., 100., 0.016);
        let (half, speed) = spring(0., 0., 100., 0.008);
        let (two, _) = spring(half, speed, 100., 0.008);
        assert!((one - two).abs() < 1e-3, "{one} vs {two}");
    }

    /// Reports that arrive unevenly still draw an even motion: each frame is
    /// placed on the straight line between the two reports around it.
    #[test]
    fn uneven_reports_draw_an_even_motion() {
        let start = Instant::now();
        let ms = |n: u64| start + Duration::from_millis(n);
        // 1 px per ms, reported after 3, 14, 5 and 11 ms.
        let reports: VecDeque<_> = [
            (ms(0), 0.),
            (ms(3), 3.),
            (ms(17), 17.),
            (ms(22), 22.),
            (ms(33), 33.),
        ]
        .into();
        for frame in [2, 8, 12, 16, 20, 25, 30] {
            let at = resample(&reports, ms(frame));
            assert!((at - frame as f32).abs() < 1e-3, "{at} at {frame} ms");
        }
        assert_eq!(resample(&reports, ms(50)), 33., "held at the newest");
    }
}
