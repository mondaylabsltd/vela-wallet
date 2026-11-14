//! The launch animation, drawn with gpui's own vector paths.
//!
//! The scene — thirteen filled shapes and two kinds of tween — is
//! [`launch_scene`](super::launch_scene), generated from
//! `docs/design/onboarding/launch/launch-scene.json`, the one description every
//! platform draws. Nothing is loaded or decoded at run time, so there is no
//! asset that can fail and no first frame to wait for: the first `render` draws
//! the first frame.
//!
//! Three rules hold the whole thing:
//!
//! * [`state`] is pure: a time in, where the mark is and how opaque each letter
//!   is out. It is held to `launch-timeline.vectors.json`, the samples every
//!   platform's timeline is checked against.
//! * The clock is the wall clock from the first frame drawn, never a frame
//!   counter, so a dropped frame skips ahead instead of stretching the motion.
//! * Paths fill NONZERO. gpui's `PathBuilder::fill()` is lyon's default, which
//!   is EVEN-ODD; the crossbars of "e" and "t" are separate contours wound the
//!   same way as the letter, and even-odd would punch them out.

use std::time::Instant;

use gpui::{
    App, Bounds, Div, FillOptions, Hsla, ParentElement, Path, PathBuilder, PathStyle, Pixels,
    Point, Styled, Window, canvas, div, px,
};

use super::launch_scene::{self as scene, Colors, Op};
use crate::theme::{
    self, LAUNCH_EXIT_CROSSFADE_MS, LAUNCH_HARD_CEILING_MS, LAUNCH_HOLD_MS, Theme, ThemeMode,
};

/// Flattening tolerance for the glyph curves, in logical pixels. lyon's
/// default (0.1) is a fifth of a device pixel on a 2× screen; this is cheap at
/// thirteen shapes and keeps the small letters' bowls round.
const TOLERANCE: f32 = 0.02;

const LETTER_COUNT: usize = scene::LETTERS.len();

/// How playback ended. Recorded for the debug log only; the user never learns
/// that anything went wrong (FR-017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Skipped,
    CeilingHit,
    ReducedMotion,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// Constructed, nothing drawn yet. The clock starts at the first frame
    /// drawn, not here, so window creation cannot eat into the motion.
    Pending,
    /// The animation, then the finished lockup held for `LAUNCH_HOLD_MS` so the
    /// brand registers rather than flashing past. `shown` is the first frame.
    Playing { shown: Instant },
    /// Cross-dissolving into Welcome, the scene frozen where it was at
    /// `scene_ms` (the finished lockup, unless a skip cut the motion short).
    Exiting {
        shown: Instant,
        since: Instant,
        scene_ms: f32,
    },
    /// Terminal. The host stops rendering the overlay.
    Dismissed,
}

pub struct LaunchAnimation {
    phase: Phase,
    outcome: Option<Outcome>,
    reduce_motion: bool,
    mode: ThemeMode,
}

impl LaunchAnimation {
    pub fn new(mode: ThemeMode, cx: &App) -> Self {
        Self::with(mode, cx.reduce_motion())
    }

    fn with(mode: ThemeMode, reduce_motion: bool) -> Self {
        Self {
            phase: Phase::Pending,
            outcome: None,
            reduce_motion,
            mode,
        }
    }

    /// Opacity the host applies to the Welcome CONTENT, so it fades in while the
    /// launch lockup fades out.
    ///
    /// The two backgrounds are the same `theme.bg_base`, which is what lets this
    /// be a true cross-dissolve: the background is continuous throughout, so
    /// there is no washed-out middle where both layers are half-transparent over
    /// the bare window.
    pub fn page_opacity(&self) -> f32 {
        self.page_opacity_at(Instant::now())
    }

    fn page_opacity_at(&self, now: Instant) -> f32 {
        match self.phase {
            Phase::Exiting { since, .. } => {
                (ms_between(since, now) / LAUNCH_EXIT_CROSSFADE_MS as f32).clamp(0., 1.)
            }
            Phase::Dismissed => 1.,
            // Welcome is composed underneath from the first frame (FR-013a) but
            // must not be visible until the dissolve starts (FR-013).
            Phase::Pending | Phase::Playing { .. } => 0.,
        }
    }

    /// Any pointer input ends playback immediately (FR-016). Latched: a skip
    /// racing the completion check must not dismiss twice.
    pub fn skip(&mut self) {
        self.skip_at(Instant::now());
    }

    fn skip_at(&mut self, now: Instant) {
        match self.phase {
            // Nothing on screen yet, so nothing to dissolve.
            Phase::Pending => self.finish(Outcome::Skipped),
            Phase::Playing { shown } => {
                let scene_ms = ms_between(shown, now).min(scene::DURATION_MS);
                self.exit(Outcome::Skipped, shown, now, scene_ms);
            }
            Phase::Exiting { .. } | Phase::Dismissed => {}
        }
    }

    /// Start the dissolve. Only ever called from `Pending` or `Playing`, so an
    /// outcome, once set, is never overwritten.
    fn exit(&mut self, outcome: Outcome, shown: Instant, since: Instant, scene_ms: f32) {
        self.outcome = Some(outcome);
        self.phase = Phase::Exiting {
            shown,
            since,
            scene_ms,
        };
    }

    /// The one way to reach `Dismissed`, so the overlay finishes exactly once.
    fn finish(&mut self, outcome: Outcome) {
        if matches!(self.phase, Phase::Dismissed) {
            return;
        }
        let outcome = *self.outcome.get_or_insert(outcome);
        // Never shown to the user (FR-017); logged because "the animation did
        // not appear" is otherwise indistinguishable from "it played and you
        // blinked", and this line is the difference when someone reports it.
        eprintln!("[vela-wallet] launch animation: {outcome:?}");
        self.phase = Phase::Dismissed;
    }

    /// Advance the phase to where it is at `now`.
    fn tick(&mut self, now: Instant) {
        match self.phase {
            Phase::Pending => {
                if self.reduce_motion {
                    // FR-019/FR-020: the finished lockup statically and NO hold
                    // — the point of the setting is less time spent on motion,
                    // so it goes straight to the dissolve.
                    self.exit(Outcome::ReducedMotion, now, now, scene::DURATION_MS);
                } else {
                    self.phase = Phase::Playing { shown: now };
                }
            }
            Phase::Playing { shown } => {
                let ms = ms_between(shown, now);
                if ms >= LAUNCH_HARD_CEILING_MS as f32 {
                    // Only after a stall (the window got no frames for seconds):
                    // too late for anything but getting out of the way.
                    self.finish(Outcome::CeilingHit);
                } else if ms >= scene::DURATION_MS + LAUNCH_HOLD_MS as f32 {
                    self.exit(Outcome::Completed, shown, now, scene::DURATION_MS);
                }
            }
            Phase::Exiting { shown, since, .. } => {
                // FR-015: the ceiling bounds the whole overlay, dissolve included.
                if ms_between(since, now) >= LAUNCH_EXIT_CROSSFADE_MS as f32
                    || ms_between(shown, now) >= LAUNCH_HARD_CEILING_MS as f32
                {
                    self.finish(Outcome::Completed);
                }
            }
            Phase::Dismissed => {}
        }
    }

    /// Where in the scene's timeline the frame drawn at `now` is.
    fn scene_ms(&self, now: Instant) -> f32 {
        match self.phase {
            Phase::Playing { shown } => ms_between(shown, now).min(scene::DURATION_MS),
            Phase::Exiting { scene_ms, .. } => scene_ms,
            Phase::Pending => 0.,
            Phase::Dismissed => scene::DURATION_MS,
        }
    }

    /// Render the overlay, or `None` once it is done.
    pub fn render(&mut self, viewport_w: f32, window: &mut Window, _cx: &mut App) -> Option<Div> {
        let now = Instant::now();
        self.tick(now);
        if matches!(self.phase, Phase::Dismissed) {
            return None;
        }
        // Every phase is time-driven — the motion, the hold, the dissolve — so
        // each needs the next frame even when nothing in the scene moves.
        window.request_animation_frame();
        Some(self.overlay(viewport_w, state(self.scene_ms(now)), now))
    }

    fn overlay(&self, viewport_w: f32, frame: Frame, now: Instant) -> Div {
        let (theme, colors) = match self.mode {
            ThemeMode::Light => (Theme::light(), scene::LIGHT),
            ThemeMode::Dark => (Theme::dark(), scene::DARK),
        };
        let (box_w, box_h) = theme::launch_box(viewport_w);

        let exiting = matches!(self.phase, Phase::Exiting { .. });
        // Fade the WHOLE overlay, not just its background: the lockup must
        // dissolve with it, or it stays opaque for the whole transition and
        // then vanishes with the element ("生硬消失", an abrupt cut).
        // `paint_path` multiplies in the element opacity, so the paths follow.
        let overlay_opacity = 1. - self.page_opacity_at(now);

        // Before the dissolve the overlay must be opaque, or Welcome shows
        // through (FR-013). During it, the page's own identically-coloured
        // background takes over, so dropping this here is what keeps the
        // backdrop continuous instead of washing out.
        let backdrop = if exiting {
            with_alpha(theme.bg_base, 0.)
        } else {
            theme.bg_base
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(backdrop)
            .opacity(overlay_opacity)
            .child(
                div().w(px(box_w)).h(px(box_h)).child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| paint(bounds, frame, colors, window),
                    )
                    .size_full(),
                ),
            )
    }
}

// ---------------------------------------------------------------------------
// The timeline
// ---------------------------------------------------------------------------

/// The scene at one moment: where the mark's moving point is (phone-canvas
/// coordinates) and how opaque each letter is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    mark: (f32, f32),
    letters: [f32; LETTER_COUNT],
}

/// The scene `ms` milliseconds after the start. Pure, and clamped at both ends.
fn state(ms: f32) -> Frame {
    let ease = cubic_bezier(
        scene::EASING[0],
        scene::EASING[1],
        scene::EASING[2],
        scene::EASING[3],
    );
    let tween = |start: f32, end: f32| ease(((ms - start) / (end - start)).clamp(0., 1.));

    let t = tween(scene::MARK_START_MS, scene::MARK_END_MS);
    let lerp = |from: f32, to: f32| from + (to - from) * t;
    let mark = (
        lerp(scene::MARK_FROM.0, scene::MARK_TO.0),
        lerp(scene::MARK_FROM.1, scene::MARK_TO.1),
    );

    let mut letters = [0.; LETTER_COUNT];
    for (opacity, letter) in letters.iter_mut().zip(scene::LETTERS) {
        *opacity = tween(letter.start_ms, letter.end_ms);
    }
    Frame { mark, letters }
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` as a function of progress.
///
/// gpui's easings are fixed curves with no cubic-bezier, and the scene's is
/// CSS's. The curve is parametric, so finding the y for a given x means
/// solving x(t) = x first: Newton's method converges in a few steps from
/// t = x on any sane curve; bisection catches the ones where the slope
/// flattens out and Newton would stall or leave 0..1.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 {
    // Each coordinate as a polynomial ((a·t + b)·t + c)·t, in f64 so the
    // solver's tolerance is far below what an f32 opacity can show.
    let coefficients = |p1: f32, p2: f32| {
        let c = 3. * f64::from(p1);
        let b = 3. * (f64::from(p2) - f64::from(p1)) - c;
        (1. - c - b, b, c)
    };
    let (ax, bx, cx) = coefficients(x1, x2);
    let (ay, by, cy) = coefficients(y1, y2);
    let x_at = move |t: f64| ((ax * t + bx) * t + cx) * t;
    let slope_at = move |t: f64| (3. * ax * t + 2. * bx) * t + cx;
    let y_at = move |t: f64| ((ay * t + by) * t + cy) * t;

    move |progress: f32| {
        if progress <= 0. {
            return 0.;
        }
        if progress >= 1. {
            return 1.;
        }
        let x = f64::from(progress);
        const EPSILON: f64 = 1e-7;

        let mut t = x;
        for _ in 0..8 {
            let error = x_at(t) - x;
            if error.abs() < EPSILON {
                return y_at(t) as f32;
            }
            let slope = slope_at(t);
            if slope.abs() < 1e-6 {
                break;
            }
            t -= error / slope;
            if !(0. ..=1.).contains(&t) {
                break;
            }
        }

        // x(t) is monotonic on 0..1 for every valid CSS curve (x1, x2 in 0..1).
        let (mut lo, mut hi) = (0., 1.);
        t = x;
        for _ in 0..64 {
            let error = x_at(t) - x;
            if error.abs() < EPSILON {
                break;
            }
            if error < 0. {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.;
        }
        y_at(t) as f32
    }
}

// ---------------------------------------------------------------------------
// The drawing
// ---------------------------------------------------------------------------

/// The large-screen framing of the scene inside a box: the phone canvas is
/// placed on the 680 × 220 canvas at `LARGE_OFFSET` × `LARGE_SCALE`, and that
/// canvas is scaled uniformly to the box and centred in it. `theme::launch_box`
/// gives the box the canvas's own aspect, so the canvas fills it exactly.
#[derive(Clone, Copy, Debug)]
struct Fit {
    scale: f32,
    x: f32,
    y: f32,
}

impl Fit {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        let (canvas_w, canvas_h) = scene::LARGE_CANVAS;
        let box_scale = (w / canvas_w).min(h / canvas_h);
        Self {
            scale: box_scale * scene::LARGE_SCALE,
            x: x + (w - canvas_w * box_scale) / 2. + scene::LARGE_OFFSET.0 * box_scale,
            y: y + (h - canvas_h * box_scale) / 2. + scene::LARGE_OFFSET.1 * box_scale,
        }
    }

    fn of(bounds: Bounds<Pixels>) -> Self {
        Self::new(
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        )
    }

    /// A phone-canvas point, in the box's coordinates.
    fn map(&self, (x, y): (f32, f32)) -> (f32, f32) {
        (self.x + x * self.scale, self.y + y * self.scale)
    }
}

/// One filled shape of the scene at a moment: its outline, the offset that
/// puts it on the phone canvas, and its colour.
struct Shape {
    ops: &'static [Op],
    offset: (f32, f32),
    rgb: u32,
    alpha: f32,
}

/// Every shape to draw at `frame`, back to front. Letters that have not begun
/// to appear are left out rather than drawn transparent.
fn shapes(frame: Frame, colors: Colors) -> impl Iterator<Item = Shape> {
    let mark = scene::MARK_PATHS.iter().map(move |path| Shape {
        ops: path.ops,
        offset: frame.mark,
        rgb: colors.get(path.fill),
        alpha: 1.,
    });
    let wordmark = colors.wordmark;
    let letters = scene::LETTERS
        .iter()
        .zip(frame.letters)
        .filter(|(_, opacity)| *opacity > 0.)
        .map(move |(letter, opacity)| Shape {
            ops: letter.ops,
            offset: (0., 0.),
            rgb: wordmark,
            alpha: opacity,
        });
    mark.chain(letters)
}

/// Walk a shape's outline in box coordinates — the one place the scene's path
/// operations are interpreted, shared by the gpui painter and the test
/// rasteriser so both draw the same geometry.
fn trace(shape: &Shape, fit: &Fit, mut emit: impl FnMut(Op)) {
    let at = |x: f32, y: f32| fit.map((x + shape.offset.0, y + shape.offset.1));
    for op in shape.ops {
        emit(match *op {
            Op::Move(x, y) => {
                let (x, y) = at(x, y);
                Op::Move(x, y)
            }
            Op::Line(x, y) => {
                let (x, y) = at(x, y);
                Op::Line(x, y)
            }
            Op::Curve(x1, y1, x2, y2, x, y) => {
                let ((x1, y1), (x2, y2), (x, y)) = (at(x1, y1), at(x2, y2), at(x, y));
                Op::Curve(x1, y1, x2, y2, x, y)
            }
            Op::Close => Op::Close,
        });
    }
}

fn paint(bounds: Bounds<Pixels>, frame: Frame, colors: Colors, window: &mut Window) {
    let fit = Fit::of(bounds);
    for shape in shapes(frame, colors) {
        // A tessellation failure would be a malformed generated path — caught
        // by the tests long before a user could see it. Skip the one shape
        // rather than the whole launch.
        if let Some(path) = tessellate(&shape, &fit, nonzero()) {
            window.paint_path(path, fill(shape.rgb, shape.alpha));
        }
    }
}

/// The fill style every shape is drawn with — NOT `PathBuilder::fill()`'s
/// default, which is lyon's even-odd (see the module notes).
fn nonzero() -> PathStyle {
    PathStyle::Fill(FillOptions::non_zero().with_tolerance(TOLERANCE))
}

fn tessellate(shape: &Shape, fit: &Fit, style: PathStyle) -> Option<Path<Pixels>> {
    let point = |x: f32, y: f32| Point::new(px(x), px(y));
    let mut pb = PathBuilder::fill().with_style(style);
    trace(shape, fit, |op| match op {
        Op::Move(x, y) => pb.move_to(point(x, y)),
        Op::Line(x, y) => pb.line_to(point(x, y)),
        Op::Curve(x1, y1, x2, y2, x, y) => {
            pb.cubic_bezier_to(point(x, y), point(x1, y1), point(x2, y2))
        }
        Op::Close => pb.close(),
    });
    pb.build().ok()
}

/// `0xRRGGBB` at `alpha`. A letter's fade is its fill's alpha.
fn fill(rgb: u32, alpha: f32) -> Hsla {
    with_alpha(gpui::rgb(rgb).into(), alpha)
}

fn with_alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

fn ms_between(start: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(start).as_secs_f32() * 1000.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // -- the timeline -------------------------------------------------------

    /// The timeline every platform is held to, sampled by the generator from
    /// `launch-scene.json`. The web's rendering of it was compared pixel for
    /// pixel with the Lottie files it replaced; matching these samples is what
    /// makes the desktop's the same motion.
    #[test]
    fn state_matches_the_shared_timeline_vectors() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/design/onboarding/launch/launch-timeline.vectors.json"
        ))
        .expect("the timeline vectors parse");

        let easing: Vec<f32> = serde_json::from_value(vectors["easing"].clone()).unwrap();
        assert_eq!(
            easing,
            scene::EASING,
            "the vectors were sampled with another easing"
        );

        let cases = vectors["cases"].as_array().expect("cases");
        assert!(
            cases.len() > 20,
            "the vectors are read ({} cases)",
            cases.len()
        );
        for case in cases {
            let ms = case["ms"].as_f64().unwrap() as f32;
            let got = state(ms);
            let (x, y) = (
                case["mark"]["x"].as_f64().unwrap() as f32,
                case["mark"]["y"].as_f64().unwrap() as f32,
            );
            assert!(
                (got.mark.0 - x).abs() <= 0.05 && (got.mark.1 - y).abs() <= 0.05,
                "{ms} ms: mark at {:?}, the vectors say ({x}, {y})",
                got.mark
            );
            let letters = case["letters"].as_array().unwrap();
            assert_eq!(letters.len(), LETTER_COUNT, "{ms} ms: letter count");
            for (i, want) in letters.iter().enumerate() {
                let want = want.as_f64().unwrap() as f32;
                assert!(
                    (got.letters[i] - want).abs() <= 0.005,
                    "{ms} ms: letter {i} ({}) at {}, the vectors say {want}",
                    scene::LETTERS[i].glyph,
                    got.letters[i]
                );
            }
        }
    }

    #[test]
    fn state_is_clamped_at_both_ends() {
        let start = state(-500.);
        assert_eq!(start.mark, scene::MARK_FROM);
        assert!(start.letters.iter().all(|&o| o == 0.));

        let settled = state(scene::SETTLED_MS);
        assert_eq!(settled, state(scene::DURATION_MS));
        assert_eq!(settled, state(1e9));
        assert_eq!(settled.mark, scene::MARK_TO);
        assert!(settled.letters.iter().all(|&o| o == 1.));
    }

    #[test]
    fn cubic_bezier_solves_known_curves() {
        let linear = cubic_bezier(0., 0., 1., 1.);
        for i in 0..=20 {
            let x = i as f32 / 20.;
            assert!((linear(x) - x).abs() < 1e-5, "linear({x}) = {}", linear(x));
        }

        // CSS `ease` at 0.5 — the value browsers produce.
        let ease = cubic_bezier(0.25, 0.1, 0.25, 1.);
        assert!(
            (ease(0.5) - 0.8024).abs() < 1e-3,
            "ease(0.5) = {}",
            ease(0.5)
        );

        // The scene's own curve: pinned at the ends, monotonic between.
        let [x1, y1, x2, y2] = scene::EASING;
        let curve = cubic_bezier(x1, y1, x2, y2);
        assert_eq!(curve(0.), 0.);
        assert_eq!(curve(1.), 1.);
        let mut last = 0.;
        for i in 1..=1000 {
            let y = curve(i as f32 / 1000.);
            assert!(y >= last, "not monotonic at {i}/1000");
            last = y;
        }

        // A curve flat enough at its start that Newton's first step leaves
        // 0..1 — the bisection must still land on the curve.
        let steep = cubic_bezier(1., 0., 1., 0.);
        let y = steep(0.3);
        assert!(y.is_finite() && (0. ..=1.).contains(&y), "steep(0.3) = {y}");
    }

    // -- the framing --------------------------------------------------------

    /// Every point of every shape, at the start and at the end, lands inside
    /// the large canvas when the box is the canvas — the scene's framing
    /// (`translate(25, 2) scale(1.8)`) is what the desktop draws.
    #[test]
    fn large_screen_framing_maps_the_scene_into_the_large_canvas() {
        let (w, h) = scene::LARGE_CANVAS;
        let fit = Fit::new(0., 0., w, h);

        // The transform itself, on two points the design names.
        let (x, y) = fit.map(scene::MARK_TO);
        assert!((x - (25. + 1.8 * 49.)).abs() < 1e-3 && (y - (2. + 1.8 * 60.)).abs() < 1e-3);
        let (x, y) = fit.map((0., 0.));
        assert!(
            (x - scene::LARGE_OFFSET.0).abs() < 1e-3 && (y - scene::LARGE_OFFSET.1).abs() < 1e-3
        );

        for ms in [0., scene::MARK_END_MS, scene::DURATION_MS] {
            let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
            let everything = Frame {
                letters: [1.; LETTER_COUNT],
                ..state(ms)
            };
            for shape in shapes(everything, scene::DARK) {
                trace(&shape, &fit, |op| {
                    let points: &[(f32, f32)] = match op {
                        Op::Move(x, y) | Op::Line(x, y) => &[(x, y)],
                        Op::Curve(x1, y1, x2, y2, x, y) => &[(x1, y1), (x2, y2), (x, y)],
                        Op::Close => &[],
                    };
                    for &(x, y) in points {
                        min = (min.0.min(x), min.1.min(y));
                        max = (max.0.max(x), max.1.max(y));
                    }
                });
            }
            assert!(
                min.0 >= 0. && min.1 >= 0. && max.0 <= w && max.1 <= h,
                "{ms} ms: the scene spans {min:?}..{max:?}, outside the {w}×{h} canvas"
            );
        }
    }

    /// The box the host gives scales the whole framing uniformly: at the
    /// minimum window (1280) the canvas is 2/3 of its authored size, and every
    /// point moves with it.
    #[test]
    fn the_framing_scales_with_the_box() {
        let (box_w, box_h) = theme::launch_box(1280.);
        let small = Fit::new(10., 20., box_w, box_h);
        let full = Fit::new(0., 0., scene::LARGE_CANVAS.0, scene::LARGE_CANVAS.1);
        let k = box_w / scene::LARGE_CANVAS.0;
        for p in [(0., 0.), scene::MARK_FROM, scene::MARK_TO, (350., 120.)] {
            let (a, b) = (small.map(p), full.map(p));
            assert!(
                (a.0 - (10. + b.0 * k)).abs() < 1e-3 && (a.1 - (20. + b.1 * k)).abs() < 1e-3,
                "{p:?}: {a:?} is not {b:?} scaled by {k}"
            );
        }
    }

    /// The finished lockup, mark to the "t", is 566.73 of the 680-wide large
    /// canvas — the number the shared fit-rule table in `theme.rs` is built on
    /// (`launch_box_matches_the_authored_proportions`), measured here off the
    /// paths themselves (curves sampled, not their control points).
    #[test]
    fn the_lockup_is_the_authored_width() {
        let fit = Fit::new(0., 0., scene::LARGE_CANVAS.0, scene::LARGE_CANVAS.1);
        let (mut left, mut right) = (f32::MAX, f32::MIN);
        for shape in shapes(state(scene::DURATION_MS), scene::DARK) {
            let mut pen = (0., 0.);
            trace(&shape, &fit, |op| {
                let mut see = |x: f32| {
                    left = left.min(x);
                    right = right.max(x);
                };
                match op {
                    Op::Move(x, y) | Op::Line(x, y) => {
                        see(x);
                        pen = (x, y);
                    }
                    Op::Curve(x1, _, x2, _, x, y) => {
                        for i in 0..=64 {
                            let t = i as f32 / 64.;
                            let u = 1. - t;
                            see(u * u * u * pen.0
                                + 3. * u * u * t * x1
                                + 3. * u * t * t * x2
                                + t * t * t * x);
                        }
                        pen = (x, y);
                    }
                    Op::Close => {}
                }
            });
        }
        let width = right - left;
        assert!(
            (width - 566.73).abs() < 0.5,
            "the finished lockup is {width} wide ({left}..{right}), authored 566.73"
        );
    }

    /// The host's box constants are the scene's canvas, so the shared fit rule
    /// and the drawing agree on what "the canvas" is.
    #[test]
    fn the_hosts_box_is_the_scenes_large_canvas() {
        use crate::theme::{
            LAUNCH_BOX_W_RATIO, LAUNCH_CANVAS_H, LAUNCH_CANVAS_W, LAUNCH_DURATION_MS,
        };
        assert_eq!((LAUNCH_CANVAS_W, LAUNCH_CANVAS_H), scene::LARGE_CANVAS);
        assert!(
            (LAUNCH_BOX_W_RATIO - scene::LARGE_CANVAS.0 / scene::LARGE_FULL_BLEED_W).abs() < 1e-6
        );
        assert_eq!(LAUNCH_DURATION_MS as f32, scene::DURATION_MS);
    }

    // -- the phases ---------------------------------------------------------

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    /// Play once, hold, dissolve, done — on the wall clock from the first
    /// frame, with Welcome invisible until the dissolve and fully there after.
    #[test]
    fn plays_once_then_holds_then_dissolves() {
        let mut launch = LaunchAnimation::with(ThemeMode::Dark, false);
        let t0 = Instant::now();
        let hold_ends = scene::DURATION_MS as u64 + LAUNCH_HOLD_MS;

        launch.tick(t0);
        assert!(matches!(launch.phase, Phase::Playing { .. }));
        assert_eq!(launch.scene_ms(t0), 0.);

        for ms in [100, 600, 1200, 1700, hold_ends - 1] {
            launch.tick(at(t0, ms));
            assert!(matches!(launch.phase, Phase::Playing { .. }), "{ms} ms");
            assert_eq!(
                launch.page_opacity_at(at(t0, ms)),
                0.,
                "{ms} ms: Welcome shows through"
            );
        }
        assert_eq!(launch.scene_ms(at(t0, 1000)), 1000.);
        assert_eq!(
            launch.scene_ms(at(t0, 2000)),
            scene::DURATION_MS,
            "the hold is the finished lockup"
        );

        launch.tick(at(t0, hold_ends));
        assert!(matches!(launch.phase, Phase::Exiting { .. }));
        let half = at(t0, hold_ends + LAUNCH_EXIT_CROSSFADE_MS / 2);
        assert!((launch.page_opacity_at(half) - 0.5).abs() < 0.01);
        assert_eq!(launch.scene_ms(half), scene::DURATION_MS);

        launch.tick(at(t0, hold_ends + LAUNCH_EXIT_CROSSFADE_MS));
        assert!(matches!(launch.phase, Phase::Dismissed));
        assert_eq!(launch.outcome, Some(Outcome::Completed));
        assert_eq!(launch.page_opacity_at(at(t0, 99_999)), 1.);
    }

    /// A skip mid-motion dissolves the scene where it stands, at once and
    /// exactly once: a second skip, or the completion check, changes nothing.
    #[test]
    fn a_skip_dissolves_from_where_the_motion_stood() {
        let mut launch = LaunchAnimation::with(ThemeMode::Light, false);
        let t0 = Instant::now();
        launch.tick(t0);
        launch.tick(at(t0, 300));
        launch.skip_at(at(t0, 300));
        assert!(matches!(launch.phase, Phase::Exiting { .. }));
        assert_eq!(
            launch.scene_ms(at(t0, 350)),
            300.,
            "frozen where the skip landed"
        );

        launch.skip_at(at(t0, 350));
        launch.tick(at(t0, 2500)); // past the point where it would have completed
        assert!(matches!(launch.phase, Phase::Dismissed));
        assert_eq!(launch.outcome, Some(Outcome::Skipped));

        launch.skip_at(at(t0, 2600));
        launch.tick(at(t0, 2700));
        assert_eq!(launch.outcome, Some(Outcome::Skipped));
    }

    /// FR-019/FR-020: the finished lockup, statically, straight into the
    /// dissolve — no motion and no hold.
    #[test]
    fn reduce_motion_shows_the_lockup_and_skips_the_hold() {
        let mut launch = LaunchAnimation::with(ThemeMode::Dark, true);
        let t0 = Instant::now();
        launch.tick(t0);
        assert!(matches!(launch.phase, Phase::Exiting { .. }));
        assert_eq!(launch.scene_ms(t0), scene::DURATION_MS);
        assert_eq!(state(launch.scene_ms(t0)), state(scene::DURATION_MS));

        launch.tick(at(t0, LAUNCH_EXIT_CROSSFADE_MS));
        assert!(matches!(launch.phase, Phase::Dismissed));
        assert_eq!(launch.outcome, Some(Outcome::ReducedMotion));
    }

    /// FR-015: a window that got no frames for seconds is not made to sit
    /// through the rest of a launch it already lost.
    #[test]
    fn the_hard_ceiling_bounds_the_whole_overlay() {
        let mut launch = LaunchAnimation::with(ThemeMode::Dark, false);
        let t0 = Instant::now();
        launch.tick(t0);
        launch.tick(at(t0, LAUNCH_HARD_CEILING_MS + 1));
        assert!(matches!(launch.phase, Phase::Dismissed));
        assert_eq!(launch.outcome, Some(Outcome::CeilingHit));

        // A dissolve that starts late is cut at the ceiling, not after it.
        let mut late = LaunchAnimation::with(ThemeMode::Dark, false);
        late.tick(t0);
        late.tick(at(t0, LAUNCH_HARD_CEILING_MS - 100));
        assert!(matches!(late.phase, Phase::Exiting { .. }));
        late.tick(at(t0, LAUNCH_HARD_CEILING_MS));
        assert!(matches!(late.phase, Phase::Dismissed));
    }

    /// The cross-dissolve, which the first implementation got wrong in a way no
    /// test could see: it faded only the overlay's BACKGROUND, so the lockup
    /// stayed fully opaque for the whole transition and then disappeared with
    /// the element. Reported from a desktop review as "生硬消失" — an abrupt cut.
    ///
    /// The invariant that catches it: overlay and page opacity must be
    /// complementary at every instant, and both must actually move.
    #[test]
    fn overlay_and_page_opacities_are_complementary_throughout() {
        let mut launch = LaunchAnimation::with(ThemeMode::Dark, true);
        let t0 = Instant::now();
        launch.tick(t0);
        let steps = 20;
        let mut seen_partial = false;
        for step in 0..=steps {
            let now = at(t0, LAUNCH_EXIT_CROSSFADE_MS * step / steps);
            let page = launch.page_opacity_at(now);
            // The expression `overlay` applies to the whole overlay.
            let overlay = 1. - page;
            assert!((page + overlay - 1.).abs() < 1e-6);
            if page > 0.05 && page < 0.95 {
                seen_partial = true;
                assert!(
                    overlay > 0.05 && overlay < 0.95,
                    "step {step}: mid-dissolve the LOCKUP must be partly transparent too"
                );
            }
        }
        assert!(
            seen_partial,
            "the dissolve never passed through a partial state"
        );
    }

    /// The transition timeline, in the order and duration a user experiences it.
    /// Pinned because these are the numbers the founder asked for by feel, and a
    /// silent change to any of them changes the product.
    #[test]
    fn transition_timeline_matches_the_agreed_shape() {
        use crate::theme::LAUNCH_DURATION_MS;

        assert_eq!(LAUNCH_DURATION_MS, 1700, "the scene's own length");
        assert_eq!(LAUNCH_HOLD_MS, 400, "hold on the finished lockup");
        assert_eq!(LAUNCH_EXIT_CROSSFADE_MS, 400, "cross-dissolve into Welcome");

        let nominal = LAUNCH_DURATION_MS + LAUNCH_HOLD_MS + LAUNCH_EXIT_CROSSFADE_MS;
        assert_eq!(nominal, 2500);
        assert!(
            LAUNCH_HARD_CEILING_MS > nominal,
            "the ceiling ({LAUNCH_HARD_CEILING_MS}) must leave room for the nominal \
             sequence ({nominal}) or a healthy launch gets cut short"
        );
        assert!(
            LAUNCH_HARD_CEILING_MS - nominal >= 400,
            "too little slack for a slow machine"
        );
    }

    // -- the picture --------------------------------------------------------

    /// The scene at `ms` as an SVG of `w` × `h`, through the same `shapes`,
    /// `Fit` and `trace` the gpui painter uses — only the rasteriser differs.
    fn svg_at(ms: f32, colors: &Colors, w: u32, h: u32, background: Option<u32>) -> String {
        use std::fmt::Write;
        let fit = Fit::new(0., 0., w as f32, h as f32);
        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
        );
        if let Some(bg) = background {
            let _ = write!(
                svg,
                r##"<rect width="{w}" height="{h}" fill="#{bg:06X}"/>"##
            );
        }
        for shape in shapes(state(ms), *colors) {
            let mut d = String::new();
            trace(&shape, &fit, |op| {
                let _ = match op {
                    Op::Move(x, y) => write!(d, "M{x} {y}"),
                    Op::Line(x, y) => write!(d, "L{x} {y}"),
                    Op::Curve(x1, y1, x2, y2, x, y) => write!(d, "C{x1} {y1} {x2} {y2} {x} {y}"),
                    Op::Close => write!(d, "Z"),
                };
            });
            let _ = write!(
                svg,
                r##"<path d="{d}" fill="#{:06X}" fill-opacity="{}" fill-rule="nonzero"/>"##,
                shape.rgb, shape.alpha
            );
        }
        svg.push_str("</svg>");
        svg
    }

    /// Rasterise [`svg_at`] with resvg, as straight (non-premultiplied) RGBA.
    fn render(
        ms: f32,
        colors: &Colors,
        w: u32,
        h: u32,
        background: Option<u32>,
    ) -> image::RgbaImage {
        let svg = svg_at(ms, colors, w, h, background);
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
            .expect("the scene's SVG parses");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap");
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for p in pixmap.pixels() {
            let c = p.demultiply();
            rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        image::RgbaImage::from_raw(w, h, rgba).expect("image")
    }

    /// The nonzero rule, seen: the middle of the final "e"'s crossbar is ink.
    /// Under even-odd the crossbar contour cancels the letter's and that spot
    /// comes out empty.
    #[test]
    fn crossbars_are_filled_not_punched_out() {
        let (w, h) = (680, 220);
        let image = render(scene::DURATION_MS, &scene::DARK, w, h, None);
        let fit = Fit::new(0., 0., w as f32, h as f32);
        for (i, letter) in scene::LETTERS.iter().enumerate() {
            if letter.glyph != "e" && letter.glyph != "t" {
                continue;
            }
            // Each crossbar is a contour of its own: the letter's last-but-one
            // `Move` starts it. Sample the centre of that contour's box.
            let contours: Vec<&[Op]> = letter
                .ops
                .split_inclusive(|op| matches!(op, Op::Close))
                .collect();
            assert!(
                contours.len() >= 2,
                "{} ({i}) has a separate crossbar contour",
                letter.glyph
            );
            let bar = contours
                .iter()
                .min_by(|a, b| area(a).total_cmp(&area(b)))
                .unwrap();
            let (cx, cy) = centre(bar);
            let (px, py) = fit.map((cx, cy));
            let alpha = image.get_pixel(px as u32, py as u32).0[3];
            assert!(
                alpha > 200,
                "{} ({i}): the crossbar's centre is {alpha}/255 ink — punched out?",
                letter.glyph
            );
        }
    }

    /// Sample points, chosen from the animation's OWN structure rather than
    /// round numbers (60 fps frame numbers, as the references are named): the
    /// mark slides over frames 12→36 and the ten letter fades run 26→72.
    /// Evenly spaced points miss both — frame 25 sits in the gap between the
    /// slide ending and the first letter appearing.
    ///
    /// frame  0 — nothing drawn yet but the mark, centred
    /// frame 24 — mid-slide
    /// frame 45 — mark landed, cascade under way
    /// frame 65 — late cascade
    /// frame 101 — the finished lockup
    const GOLDEN_FRAMES: [u32; 5] = [0, 24, 45, 65, 101];
    const GOLDEN_SIZE: (u32, u32) = (340, 110);

    fn frame_ms(frame: u32) -> f32 {
        frame as f32 * 1000. / 60.
    }

    /// Premultiplied RGBA, the encoding of the references in `tests/golden/`
    /// (they were first written by the ThorVG player this replaced, from its
    /// premultiplied buffer, and are compared in that encoding).
    fn render_premultiplied(ms: f32, colors: &Colors) -> image::RgbaImage {
        let (w, h) = GOLDEN_SIZE;
        let svg = svg_at(ms, colors, w, h, None);
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
            .expect("the scene's SVG parses");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).expect("pixmap");
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        image::RgbaImage::from_raw(w, h, pixmap.data().to_vec()).expect("image")
    }

    /// How far apart two pictures are: the mean channel difference (of 255),
    /// and how many pixels are off by more than a soft edge can explain.
    fn distance(a: &image::RgbaImage, b: &image::RgbaImage) -> (f64, usize) {
        let mut sum = 0u64;
        let mut far = 0usize;
        for (x, y) in a.pixels().zip(b.pixels()) {
            let worst =
                x.0.iter()
                    .zip(y.0.iter())
                    .map(|(p, q)| p.abs_diff(*q))
                    .max()
                    .unwrap_or(0);
            sum +=
                x.0.iter()
                    .zip(y.0.iter())
                    .map(|(p, q)| u64::from(p.abs_diff(*q)))
                    .sum::<u64>();
            if worst > 64 {
                far += 1;
            }
        }
        (sum as f64 / (a.len() as f64), far)
    }

    /// Golden frames: the scene, drawn at fixed points, against the pictures
    /// the Lottie player drew at the same points before it was removed. A
    /// different rasteriser, so the tolerance is for anti-aliasing at edges —
    /// a shape out of place, a letter at the wrong opacity, or a crossbar
    /// punched out is far outside it (see `golden_comparison_can_actually_fail`).
    ///
    /// Run with `VELA_UPDATE_GOLDEN=1` to rewrite the references; review the
    /// diff, because a change here means the animation changed. With
    /// `VELA_LAUNCH_PREVIEW=<dir>`, also writes each frame at the full canvas
    /// size over the app's background, to look at.
    #[test]
    fn golden_frames_match_the_committed_references() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        let update = std::env::var("VELA_UPDATE_GOLDEN").is_ok();
        let preview = std::env::var_os("VELA_LAUNCH_PREVIEW").map(std::path::PathBuf::from);

        let mut mismatches = Vec::new();
        for (appearance, colors, theme) in [
            ("dark", scene::DARK, Theme::dark()),
            ("light", scene::LIGHT, Theme::light()),
        ] {
            for frame in GOLDEN_FRAMES {
                let ms = frame_ms(frame);
                let actual = render_premultiplied(ms, &colors);
                let path = dir.join(format!("launch-{appearance}-f{frame:03}.png"));

                if let Some(preview) = &preview {
                    let bg = gpui::Rgba::from(theme.bg_base);
                    let bg = ((bg.r * 255.).round() as u32) << 16
                        | ((bg.g * 255.).round() as u32) << 8
                        | (bg.b * 255.).round() as u32;
                    let (w, h) = scene::LARGE_CANVAS;
                    render(ms, &colors, w as u32, h as u32, Some(bg))
                        .save(preview.join(format!("desktop-{appearance}-f{frame:03}.png")))
                        .expect("write preview");
                }
                if update {
                    actual.save(&path).expect("write golden");
                    continue;
                }

                let expected = image::open(&path)
                    .unwrap_or_else(|e| {
                        panic!("missing golden {path:?}: {e} — run with VELA_UPDATE_GOLDEN=1")
                    })
                    .to_rgba8();
                assert_eq!(expected.dimensions(), actual.dimensions(), "{path:?}: size");
                let (mean, far) = distance(&expected, &actual);
                eprintln!("{appearance} @frame {frame}: mean {mean:.3}/255, {far} px far off");
                if mean > MAX_MEAN || far > MAX_FAR {
                    mismatches.push(format!(
                        "{appearance} @frame {frame}: mean {mean:.3}/255, {far} px far off"
                    ));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "golden frames drifted:\n  {}\nIf the scene changed on purpose, rerun with VELA_UPDATE_GOLDEN=1 and review the images.",
            mismatches.join("\n  ")
        );
    }

    const MAX_MEAN: f64 = 0.75;
    const MAX_FAR: usize = 20;

    /// The comparison must be able to fail: a frame two 60 fps frames early,
    /// at every sample point where something is moving, is outside it.
    #[test]
    fn golden_comparison_can_actually_fail() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        for frame in [24, 45, 65] {
            let expected = image::open(dir.join(format!("launch-dark-f{frame:03}.png")))
                .expect("golden")
                .to_rgba8();
            let early = render_premultiplied(frame_ms(frame - 2), &scene::DARK);
            let (mean, far) = distance(&expected, &early);
            assert!(
                mean > MAX_MEAN || far > MAX_FAR,
                "frame {frame} vs {}: mean {mean:.3}, {far} px — the tolerance cannot see motion",
                frame - 2
            );
        }
    }

    /// The nonzero rule in gpui's OWN tessellation, which is what the window
    /// draws: the "e"s and the "t" cover more ink under the style `paint` uses
    /// than under `PathBuilder::fill()`'s even-odd default, by about the area
    /// of the crossbar the default would punch out. (The resvg goldens prove
    /// the geometry; only this proves the fill rule the app actually uses.)
    #[test]
    fn gpui_tessellation_fills_the_crossbars() {
        let fit = Fit::new(0., 0., scene::LARGE_CANVAS.0, scene::LARGE_CANVAS.1);
        let ink = |path: Path<Pixels>| -> f32 {
            path.vertices
                .chunks(3)
                .map(|t| {
                    let p =
                        |i: usize| (f32::from(t[i].xy_position.x), f32::from(t[i].xy_position.y));
                    let ((ax, ay), (bx, by), (cx, cy)) = (p(0), p(1), p(2));
                    ((bx - ax) * (cy - ay) - (cx - ax) * (by - ay)).abs() / 2.
                })
                .sum()
        };
        let mut checked = 0;
        // At the end every letter is drawn, in order, after the mark.
        let drawn = shapes(state(scene::DURATION_MS), scene::DARK).skip(scene::MARK_PATHS.len());
        for (letter, shape) in scene::LETTERS.iter().zip(drawn) {
            if letter.glyph != "e" && letter.glyph != "t" {
                continue;
            }
            let ours = ink(tessellate(&shape, &fit, nonzero()).expect("tessellates"));
            let default_style = PathBuilder::fill().style;
            let default = ink(tessellate(&shape, &fit, default_style).expect("tessellates"));
            // The crossbar on its own: the letter's smallest contour.
            let bar = letter
                .ops
                .split_inclusive(|op| matches!(op, Op::Close))
                .min_by(|a, b| area(a).total_cmp(&area(b)))
                .expect("contours");
            let bar = Shape { ops: bar, ..shape };
            let bar = ink(tessellate(&bar, &fit, nonzero()).expect("tessellates"));
            // Where the bar overlaps the letter, even-odd leaves a hole and
            // nonzero does not; that overlap is a real part of the bar.
            assert!(
                ours - default > bar * 0.25,
                "{}: {ours:.1} px² of ink nonzero vs {default:.1} even-odd, bar {bar:.1} — \
                 is the crossbar punched out?",
                letter.glyph
            );
            checked += 1;
        }
        assert_eq!(checked, 3, "two e's and a t");
    }

    fn points(ops: &[Op]) -> Vec<(f32, f32)> {
        ops.iter()
            .filter_map(|op| match *op {
                Op::Move(x, y) | Op::Line(x, y) | Op::Curve(_, _, _, _, x, y) => Some((x, y)),
                Op::Close => None,
            })
            .collect()
    }

    fn area(ops: &[Op]) -> f32 {
        let p = points(ops);
        let (x0, x1) = p
            .iter()
            .fold((f32::MAX, f32::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
        let (y0, y1) = p
            .iter()
            .fold((f32::MAX, f32::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
        (x1 - x0) * (y1 - y0)
    }

    fn centre(ops: &[Op]) -> (f32, f32) {
        let p = points(ops);
        let (x0, x1) = p
            .iter()
            .fold((f32::MAX, f32::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
        let (y0, y1) = p
            .iter()
            .fold((f32::MAX, f32::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
        ((x0 + x1) / 2., (y0 + y1) / 2.)
    }
}
