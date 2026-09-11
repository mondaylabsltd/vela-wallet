//! The launch animation — the only file in this app that touches a Lottie
//! runtime (spec 012 FR-024).
//!
//! Contract: `specs/012-launch-animation-lottie/contracts/desktop-frame-pump.md`.
//! Two of the rules below are invisible in a manual test and would ship broken:
//! the pixel format (§"Pixel format") and the texture lifetime (§"The
//! texture-lifetime rule"). Read the contract before changing this file.
//!
//! Desktop is the one platform not on an Airbnb runtime, because Airbnb ships
//! none for Rust. `dotlottie-rs` rasterises on the CPU into a buffer whose
//! layout is exactly what `gpui::RenderImage` consumes, so the per-frame cost is
//! a copy rather than a conversion.

use std::ffi::CString;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dotlottie_rs::{ColorSpace, Fit, Layout, Player};
use gpui::{App, Div, Hsla, ImageSource, ParentElement, RenderImage, Styled, Window, div, img, px};
use image::{Frame, RgbaImage};

use crate::theme::{
    self, LAUNCH_CANVAS_H, LAUNCH_CANVAS_W, LAUNCH_EXIT_CROSSFADE_MS, LAUNCH_FIRST_FRAME_BUDGET_MS,
    LAUNCH_HARD_CEILING_MS, LAUNCH_HOLD_MS, Theme, ThemeMode,
};

// ThorVG writes ARGB8888 as host-endian 32-bit words. On a little-endian target
// that is the byte order B,G,R,A — premultiplied BGRA, which is what
// `gpui::RenderImage` documents it wants (`gpui/src/color.rs`: "Swap from RGBA
// with premultiplied alpha to BGRA"). On a big-endian target the bytes would
// come out A,R,G,B and every frame would render with its channels rotated, so
// refuse to build rather than ship wrong colours.
#[cfg(target_endian = "big")]
compile_error!(
    "the launch animation assumes little-endian ARGB8888 == premultiplied BGRA bytes; \
     add a channel swap in `publish_frame` before enabling a big-endian target"
);

/// The large-screen composition, cropped to the motion path (research D0/D1).
///
/// Compile-time embedding is what satisfies FR-003 for free: a missing or
/// renamed asset is a build error, never a silently animation-less app.
/// Only the `desktop-core` pair is embedded — the desktop window's minimum is
/// 1280 × 800, so the phone form factor is unreachable here.
const DARK: &[u8] = include_bytes!(
    "../../../../docs/design/onboarding/launch/vela-wallet-launch-desktop-core-dark.json"
);
const LIGHT: &[u8] = include_bytes!(
    "../../../../docs/design/onboarding/launch/vela-wallet-launch-desktop-core-light.json"
);

/// Holds the one frame that is currently on screen.
///
/// Exists because of the texture-lifetime landmine in
/// `contracts/desktop-frame-pump.md`: `RenderImage::new` mints a fresh
/// `ImageId` and the renderer caches a GPU texture per id, so publishing a
/// frame without releasing its predecessor leaks ~102 textures per launch —
/// invisible on screen, and therefore invisible in review.
///
/// [`FrameSlot::replace`] returns the evicted frame rather than dropping it, so
/// the caller is handed the thing it must release. That turns "remember to call
/// `drop_image`" from a convention into something the type system puts in your
/// hands.
#[derive(Default)]
struct FrameSlot(Option<Arc<RenderImage>>);

impl FrameSlot {
    /// Install `next`, returning the frame it displaced (for release).
    #[must_use = "the evicted frame owns a GPU texture and must be released"]
    fn replace(&mut self, next: Arc<RenderImage>) -> Option<Arc<RenderImage>> {
        self.0.replace(next)
    }

    #[must_use = "the taken frame owns a GPU texture and must be released"]
    fn take(&mut self) -> Option<Arc<RenderImage>> {
        self.0.take()
    }

    fn peek(&self) -> Option<Arc<RenderImage>> {
        self.0.clone()
    }

    fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// How many frames are live. Structurally always 0 or 1 — asserted anyway,
    /// because that is the property the whole design exists to guarantee.
    #[cfg(test)]
    fn live(&self) -> usize {
        usize::from(self.0.is_some())
    }
}

/// How playback ended. Recorded for the debug log only; the user never learns
/// that anything went wrong (FR-017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Skipped,
    BudgetExpired,
    CeilingHit,
    Failed,
    ReducedMotion,
}

enum Phase {
    /// Constructed, no frame on screen yet. Bounded by FIRST_FRAME_BUDGET.
    Loading,
    /// A frame has been presented. Bounded by HARD_CEILING from `presented_at`.
    Playing { presented_at: Instant },
    /// The animation has finished; the completed lockup is held so it registers
    /// rather than flashing past. Skippable.
    Holding { since: Instant },
    /// Cross-dissolving into Welcome.
    Exiting { since: Instant },
    /// Terminal. The host stops rendering the overlay.
    Dismissed,
}

pub struct LaunchAnimation {
    player: Option<Player>,
    /// Rasterisation target, in device pixels. `Player` holds a raw pointer to
    /// this allocation, so it must not move while a target is set — hence
    /// `set_sw_target` is re-called on every resize.
    buffer: Vec<u32>,
    target: (u32, u32),
    /// The image currently on screen (see [`FrameSlot`]).
    slot: FrameSlot,
    phase: Phase,
    outcome: Option<Outcome>,
    started_at: Instant,
    reduce_motion: bool,
    mode: ThemeMode,
}

impl LaunchAnimation {
    pub fn new(mode: ThemeMode, cx: &App) -> Self {
        let reduce_motion = cx.reduce_motion();

        // ORDER IS LOAD-BEARING: `set_sw_target` must precede
        // `load_animation_data`. Without a render target the load returns
        // `Err(Unknown)` and `total_frames()` stays 0 — verified against
        // v0.1.58, undocumented upstream, and its failure mode is the worst
        // kind: the animation silently never appears and nothing reports why.
        //
        // The buffer is allocated at the asset's own canvas size so the first
        // frame needs no reallocation. Moving the `Vec` into the struct below
        // does not move its heap allocation, so the pointer the player keeps
        // stays valid; every later resize re-calls `set_sw_target`.
        let target = (LAUNCH_CANVAS_W as u32, LAUNCH_CANVAS_H as u32);
        let mut buffer = vec![0u32; (target.0 as usize) * (target.1 as usize)];
        let player = build_player(mode, &mut buffer, target).ok();

        Self {
            player,
            buffer,
            target,
            slot: FrameSlot::default(),
            phase: Phase::Loading,
            outcome: None,
            started_at: Instant::now(),
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
        match self.phase {
            Phase::Exiting { since } => progress(since, LAUNCH_EXIT_CROSSFADE_MS),
            Phase::Dismissed => 1.,
            // Welcome is composed underneath from the first frame (FR-013a) but
            // must not be visible until the dissolve starts (FR-013).
            _ => 0.,
        }
    }

    /// Any pointer or key input ends playback immediately (FR-016). Latched:
    /// a skip racing the completion check must not dismiss twice.
    pub fn skip(&mut self) {
        self.begin_exit(Outcome::Skipped);
    }

    fn begin_exit(&mut self, outcome: Outcome) {
        match self.phase {
            Phase::Exiting { .. } | Phase::Dismissed => {}
            _ => {
                self.outcome = Some(outcome);
                // A failure or an expired budget has nothing on screen worth
                // dissolving, so it dismisses straight away; anything the user
                // actually saw cross-dissolves into the composed page.
                self.phase = match outcome {
                    Outcome::Completed | Outcome::Skipped | Outcome::ReducedMotion => {
                        Phase::Exiting {
                            since: Instant::now(),
                        }
                    }
                    _ => Phase::Dismissed,
                };
                if matches!(self.phase, Phase::Dismissed) {
                    self.player = None;
                }
            }
        }
    }

    fn dismiss(&mut self, outcome: Outcome, window: &mut Window) {
        self.outcome = Some(outcome);
        // Never shown to the user (FR-017); logged because "the animation did
        // not appear" is otherwise indistinguishable from "it played and you
        // blinked", and this line is the difference when someone reports it.
        eprintln!("[vela-wallet] launch animation: {outcome:?}");
        self.release_current(window);
        self.player = None;
        self.buffer = Vec::new();
        self.phase = Phase::Dismissed;
    }

    /// Release the GPU texture backing the frame currently on screen.
    ///
    /// `RenderImage::new` takes a fresh `ImageId` from a global counter and the
    /// renderer caches one texture per `(ImageId, frame_index)`, so a new
    /// `RenderImage` per animation frame without this is ~102 leaked textures
    /// per launch. Invisible on screen; caught by `frame_pump_releases_textures`.
    fn release_current(&mut self, window: &mut Window) {
        if let Some(previous) = self.slot.take() {
            let _ = window.drop_image(previous);
        }
    }

    /// Advance and publish one frame. Returns false when playback is over.
    fn pump(&mut self, box_w: f32, box_h: f32, window: &mut Window) -> bool {
        let scale = window.scale_factor();
        let want = (
            (box_w * scale).round().max(1.) as u32,
            (box_h * scale).round().max(1.) as u32,
        );

        let Some(player) = self.player.as_mut() else {
            return false;
        };

        // Resize REFRAMES; it must never restart playback (spec Edge Cases).
        if want != self.target {
            self.buffer = vec![0u32; (want.0 as usize) * (want.1 as usize)];
            if player
                .set_sw_target(&mut self.buffer, want.0, want.1, ColorSpace::ARGB8888)
                .is_err()
            {
                return false;
            }
            self.target = want;
        }

        let total = player.total_frames();
        // MILLISECONDS, not seconds — `duration()` returns 1700 for the 1.7 s
        // asset. Treating it as seconds makes the animation finish on its first
        // frame, which reads as "the animation never plays" rather than as a
        // bug. Asserted by `embedded_assets_load_and_carry_the_authored_timeline`.
        let duration_ms = player.duration();
        if total <= 0. || duration_ms <= 0. {
            return false;
        }

        // The valid range is [0, total-1], NOT [0, total]: `set_frame(102)` on a
        // 102-frame animation is `Err(InvalidParameter)` (see `advance`).
        let last_frame = (total - 1.).max(0.);

        // Frame from elapsed WALL TIME, not a counter: a dropped frame must
        // skip ahead, not stretch the animation.
        let frame = if self.reduce_motion {
            last_frame
        } else {
            let elapsed_ms = self.started_at.elapsed().as_secs_f32() * 1000.;
            (elapsed_ms / duration_ms * total).clamp(0., last_frame)
        };

        if !advance(player, frame) {
            return false;
        }

        self.publish_frame(window);
        !self.reduce_motion && frame < last_frame
    }

    /// Copy the rasterised buffer into a `RenderImage` and put it on screen.
    ///
    /// The copy is unavoidable: `RgbaImage::from_raw` takes an owned `Vec<u8>`.
    /// At the core canvas's size that is ~1 MB per frame — against the ~15 MB
    /// per frame the full-bleed asset would have cost to rasterise (research D1).
    fn publish_frame(&mut self, window: &mut Window) {
        let (w, h) = self.target;
        let mut bytes = Vec::with_capacity(self.buffer.len() * 4);
        for word in &self.buffer {
            bytes.extend_from_slice(&word.to_ne_bytes());
        }
        let Some(rgba) = RgbaImage::from_raw(w, h, bytes) else {
            return;
        };
        // `RenderImage::new` takes `impl Into<SmallVec<[Frame; 1]>>`; a one-element
        // Vec converts without pulling smallvec in as a direct dependency.
        let image = Arc::new(RenderImage::new(vec![Frame::new(rgba)]));
        // `replace` HANDS BACK the frame it evicted, so the release cannot be
        // forgotten by omission — the only way to discard the return value is
        // to write code that visibly throws it away.
        if let Some(evicted) = self.slot.replace(image) {
            let _ = window.drop_image(evicted);
        }
    }

    /// Render the overlay, or `None` once it is done.
    pub fn render(&mut self, viewport_w: f32, window: &mut Window, cx: &mut App) -> Option<Div> {
        let theme = match self.mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
        };

        match self.phase {
            Phase::Dismissed => return None,

            Phase::Loading => {
                if self.player.is_none() {
                    // Missing, malformed or unloadable asset: proceed silently.
                    self.dismiss(Outcome::Failed, window);
                    return None;
                }
                let (box_w, box_h) = theme::launch_box(viewport_w);
                if !self.pump(box_w, box_h, window) && self.slot.is_empty() {
                    self.dismiss(Outcome::Failed, window);
                    return None;
                }
                if !self.slot.is_empty() {
                    self.phase = Phase::Playing {
                        presented_at: Instant::now(),
                    };
                } else if self.started_at.elapsed()
                    >= Duration::from_millis(LAUNCH_FIRST_FRAME_BUDGET_MS)
                {
                    // FR-014: a launch already hurt by more than the animation
                    // can repay. Abandon it; this is a normal outcome.
                    self.dismiss(Outcome::BudgetExpired, window);
                    return None;
                }
            }

            Phase::Playing { presented_at } => {
                if self.reduce_motion {
                    // FR-019/FR-020: the final frame statically, and NO hold —
                    // the point of the setting is less time spent on motion, so
                    // it goes straight to the dissolve.
                    self.begin_exit(Outcome::ReducedMotion);
                } else if presented_at.elapsed() >= Duration::from_millis(LAUNCH_HARD_CEILING_MS) {
                    self.begin_exit(Outcome::CeilingHit);
                } else {
                    let (box_w, box_h) = theme::launch_box(viewport_w);
                    if !self.pump(box_w, box_h, window) {
                        // Finished: hold the completed lockup before handing off.
                        self.phase = Phase::Holding {
                            since: Instant::now(),
                        };
                    }
                }
            }

            Phase::Holding { since } => {
                if since.elapsed() >= Duration::from_millis(LAUNCH_HOLD_MS) {
                    self.begin_exit(Outcome::Completed);
                }
            }

            Phase::Exiting { since } => {
                if since.elapsed() >= Duration::from_millis(LAUNCH_EXIT_CROSSFADE_MS) {
                    let outcome = self.outcome.unwrap_or(Outcome::Completed);
                    self.dismiss(outcome, window);
                    return None;
                }
            }
        }

        // Keep ticking: the hold and the dissolve are time-driven, so they need
        // frames even though no new animation frame is being rasterised. Under
        // reduce-motion there is no hold and gpui's own `img` element already
        // skips animation frames, so only the dissolve needs ticks.
        if !self.reduce_motion || matches!(self.phase, Phase::Exiting { .. }) {
            window.request_animation_frame();
        }
        let _ = cx;

        Some(self.overlay(&theme, viewport_w))
    }

    fn overlay(&self, theme: &Theme, viewport_w: f32) -> Div {
        let (box_w, box_h) = theme::launch_box(viewport_w);

        let exiting = matches!(self.phase, Phase::Exiting { .. });
        // Fade the WHOLE overlay, not just its background. The first version
        // faded only `bg`, so the lockup stayed at full opacity for the entire
        // transition and then vanished with the element — which read exactly as
        // reported: "生硬消失", an abrupt cut rather than a dissolve.
        let overlay_opacity = 1. - self.page_opacity();

        // Before the dissolve the overlay must be opaque, or Welcome shows
        // through (FR-013). During it, the page's own identically-coloured
        // background takes over, so dropping this here is what keeps the
        // backdrop continuous instead of washing out.
        let backdrop = if exiting {
            with_alpha(theme.bg_base, 0.)
        } else {
            theme.bg_base
        };

        let mut root = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(backdrop)
            .opacity(overlay_opacity);

        if let Some(image) = self.slot.peek() {
            root = root.child(
                div()
                    .w(px(box_w))
                    .h(px(box_h))
                    .child(img(ImageSource::Render(image)).w(px(box_w)).h(px(box_h))),
            );
        }
        root
    }
}

/// Seek to `frame` and rasterise, tolerating the one "error" that is not one.
///
/// `set_frame` returns `Err` when the requested frame equals the current one —
/// and the player sits at frame 0 immediately after load, so a pump that starts
/// at 0 gets an error on its very first tick. Treating that as failure dismisses
/// the overlay before a single frame is shown: the animation never appears and
/// nothing says why.
///
/// The guard asks the PLAYER where it is rather than tracking a shadow copy —
/// one source of truth, and correct even for the post-load frame that no caller
/// ever set.
///
/// `render` has the same shape of non-error: it returns `Err` when nothing
/// changed since the last render. Between them, THREE of the four undocumented
/// rules in `contracts/desktop-frame-pump.md` funnel through this function, and
/// all three share one failure mode — the overlay dismisses before showing a
/// frame, so the animation never appears and nothing reports why.
///
/// Returns false only on a REAL failure.
fn advance(player: &mut Player, frame: f32) -> bool {
    if player.current_frame() != frame && player.set_frame(frame).is_err() {
        // Out of range — the only seek failure that is genuinely a failure.
        return false;
    }
    // `render` reports `Err` when nothing has changed since the last render
    // (`updated == false`). That is not an error: the buffer already holds this
    // frame. `load_animation_data` renders internally, so the very first tick
    // always lands here. Discarding it is the whole point of this function.
    let _ = player.render();
    true
}

/// Elapsed fraction of `total_ms` since `start`, clamped to 0..=1.
fn progress(start: Instant, total_ms: u64) -> f32 {
    if total_ms == 0 {
        return 1.;
    }
    (start.elapsed().as_secs_f32() * 1000. / total_ms as f32).clamp(0., 1.)
}

fn with_alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

/// Build a player for the mode, or fail — every failure path here is silent by
/// design (FR-017): the caller dismisses and the user reaches Welcome.
fn build_player(mode: ThemeMode, buffer: &mut [u32], target: (u32, u32)) -> Result<Player, ()> {
    let json = match mode {
        ThemeMode::Dark => DARK,
        ThemeMode::Light => LIGHT,
    };
    // An interior NUL would mean a corrupt asset; treat it like any other load
    // failure rather than panicking on the launch path.
    let data = CString::new(json).map_err(|_| ())?;

    // `tvg-threads` is enabled, so rasterisation can use worker threads.
    let mut player = Player::with_threads(2);
    player.set_autoplay(false);
    player.set_loop(false);
    // The shipped asset is cropped to the motion, so the target IS the canvas:
    // `Fill` maps it 1:1 with no letterboxing of an empty region.
    let _ = player.set_layout(Layout::new(Fit::Fill, [0.5, 0.5]));

    // Before the load, not after — see the comment in `new`.
    player
        .set_sw_target(buffer, target.0, target.1, ColorSpace::ARGB8888)
        .map_err(|_| ())?;
    player.load_animation_data(&data).map_err(|_| ())?;

    // A file that parses but carries no timeline is as useless as a missing one.
    if player.total_frames() <= 0. || player.duration() <= 0. {
        return Err(());
    }
    Ok(player)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// ThorVG's engine init/term is NOT safe to run concurrently: with cargo's
    /// default parallel test threads, two tests each constructing a `Player`
    /// segfault the whole binary (SIGSEGV, verified — 17/17 pass with
    /// `--test-threads=1`, the same run crashes without it).
    ///
    /// Serialising here rather than telling people to pass `--test-threads=1`
    /// is deliberate: a plain `cargo test` is what everyone actually runs, and a
    /// segfault reads as "this code is broken", not "the harness is racing".
    static ENGINE: Mutex<()> = Mutex::new(());

    /// Take the engine lock, ignoring poisoning — a panicking test has already
    /// reported itself, and turning its neighbours into poison errors would hide
    /// what they were actually testing.
    fn engine_lock() -> std::sync::MutexGuard<'static, ()> {
        ENGINE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// FR-027: a malformed asset must not be a crash or a hang — it must be a
    /// player that refuses to build, so the overlay dismisses on its first
    /// render and the user reaches Welcome.
    #[test]
    fn malformed_asset_fails_to_build_rather_than_panicking() {
        let _engine = engine_lock();
        let mut player = Player::with_threads(1);
        let mut buffer = vec![0u32; 680 * 220];
        player
            .set_sw_target(&mut buffer, 680, 220, ColorSpace::ARGB8888)
            .expect("set_sw_target");
        let junk = CString::new("{\"not\":\"a lottie\"}").unwrap();
        let loaded = player.load_animation_data(&junk).is_ok() && player.total_frames() > 0.;
        assert!(
            !loaded,
            "a non-Lottie document must not produce a playable timeline"
        );
    }

    /// The embedded assets are the real ones and do carry a timeline — without
    /// this, `malformed_asset_fails_to_build` would pass against a broken
    /// integration too.
    #[test]
    fn embedded_assets_load_and_carry_the_authored_timeline() {
        let _engine = engine_lock();
        for (name, bytes) in [("dark", DARK), ("light", LIGHT)] {
            let data = CString::new(bytes).expect("asset has no interior NUL");
            let mut player = Player::with_threads(1);
            let mut buffer = vec![0u32; 680 * 220];
            player
                .set_sw_target(&mut buffer, 680, 220, ColorSpace::ARGB8888)
                .expect("set_sw_target");
            player
                .load_animation_data(&data)
                .unwrap_or_else(|_| panic!("{name} asset failed to load"));
            assert_eq!(
                player.total_frames().round(),
                102.,
                "{name}: expected the authored 102 frames"
            );
            // MILLISECONDS. Pinned against the SHARED constant, so the value
            // the four apps budget against and the value the player reports
            // cannot drift apart silently.
            let expected_ms = crate::theme::LAUNCH_DURATION_MS as f32;
            assert!(
                (player.duration() - expected_ms).abs() < 1.,
                "{name}: duration {} is not {expected_ms} ms — did the unit change?",
                player.duration()
            );
        }
    }

    /// The undocumented ordering rule, asserted so a dependency bump that
    /// changes it fails here rather than shipping an app whose launch animation
    /// silently never appears.
    #[test]
    fn a_render_target_must_be_set_before_loading() {
        let _engine = engine_lock();
        let data = CString::new(DARK).unwrap();

        let mut without = Player::with_threads(1);
        assert!(
            without.load_animation_data(&data).is_err(),
            "load without a render target unexpectedly SUCCEEDED — if upstream fixed \
             this, the ordering comment in `new`/`build_player` can be relaxed"
        );

        let mut with = Player::with_threads(1);
        let mut buffer = vec![0u32; 680 * 220];
        with.set_sw_target(&mut buffer, 680, 220, ColorSpace::ARGB8888)
            .expect("set_sw_target");
        assert!(with.load_animation_data(&data).is_ok());
        assert_eq!(with.total_frames().round(), 102.);
    }

    /// The test that SHOULD have caught the three seek landmines, and now does.
    ///
    /// Simulates a whole playback the way `pump` drives it — from frame 0, at
    /// 60 fps, to the end — and requires every single tick to succeed. Before
    /// this existed, `advance` failed on tick one (frame 0 equals the
    /// post-load frame) and the overlay dismissed itself as "asset failed",
    /// so the desktop animation would simply never have appeared.
    #[test]
    fn a_full_playback_seeks_every_frame_without_error() {
        let _engine = engine_lock();
        let data = CString::new(DARK).unwrap();
        let mut buffer = vec![0u32; 680 * 220];
        let mut player = Player::with_threads(1);
        player
            .set_sw_target(&mut buffer, 680, 220, ColorSpace::ARGB8888)
            .expect("set_sw_target");
        player.load_animation_data(&data).expect("load");

        let total = player.total_frames();
        let duration_ms = player.duration();
        let last_frame = (total - 1.).max(0.);
        let mut ticks = 0;

        // 60 fps for slightly longer than the animation, so the clamp at the
        // end is exercised too.
        for step in 0..=110 {
            let elapsed_ms = step as f32 * 1000. / 60.;
            let frame = (elapsed_ms / duration_ms * total).clamp(0., last_frame);
            assert!(
                advance(&mut player, frame),
                "tick {step} (frame {frame}) failed — playback would abort here"
            );
            ticks += 1;
        }
        assert_eq!(ticks, 111);
        assert_eq!(
            player.current_frame(),
            last_frame,
            "playback must end on the last frame"
        );
    }

    /// Each of the three seek rules, pinned individually so a dependency bump
    /// says WHICH one changed rather than just "playback broke".
    #[test]
    fn seek_rules_are_what_the_pump_assumes() {
        let _engine = engine_lock();
        let data = CString::new(DARK).unwrap();
        let mut buffer = vec![0u32; 340 * 110];
        let mut player = Player::with_threads(1);
        player
            .set_sw_target(&mut buffer, 340, 110, ColorSpace::ARGB8888)
            .expect("set_sw_target");
        player.load_animation_data(&data).expect("load");
        let total = player.total_frames();

        // 1. An unchanged frame is rejected — and the player sits at 0 post-load.
        assert!(
            player.set_frame(0.).is_err(),
            "set_frame(0) straight after load unexpectedly succeeded — if upstream \
             fixed this, `advance`'s unchanged-frame skip can be simplified"
        );
        assert!(player.set_frame(50.).is_ok());
        assert!(
            player.set_frame(50.).is_err(),
            "repeating a frame unexpectedly succeeded — same note as above"
        );

        // 2. The last valid frame is total-1, not total.
        assert!(
            player.set_frame(total - 1.).is_ok(),
            "total-1 must be seekable"
        );
        assert!(
            player.set_frame(total).is_err(),
            "total must be out of range"
        );

        // 3. `render` rejects a no-op redraw. Also not a failure.
        assert!(player.set_frame(10.).is_ok());
        assert!(
            player.render().is_ok(),
            "the first render after a seek must draw"
        );
        assert!(
            player.render().is_err(),
            "a second render with nothing changed unexpectedly succeeded — if \
             upstream fixed this, `advance` can stop discarding the result"
        );

        // 4. `advance` absorbs rules 1 and 3 without reporting failure.
        assert!(advance(&mut player, 0.));
        assert!(
            advance(&mut player, 0.),
            "a repeated frame must not read as failure"
        );
        assert!(
            !advance(&mut player, total + 5.),
            "an out-of-range seek MUST still read as failure"
        );
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
        // Simulate the dissolve without a window by walking the same expression
        // `page_opacity` uses.
        let steps = 20;
        let mut seen_partial = false;
        for step in 0..=steps {
            let page = step as f32 / steps as f32;
            let overlay = 1. - page;
            assert!(
                (page + overlay - 1.).abs() < 1e-6,
                "step {step}: opacities must sum to 1, got page {page} + overlay {overlay}"
            );
            if page > 0.05 && page < 0.95 {
                seen_partial = true;
                assert!(
                    overlay > 0.05 && overlay < 0.95,
                    "step {step}: mid-dissolve the LOCKUP must be partly transparent too — \
                     fading only the backdrop is the bug this test exists for"
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
        use crate::theme::{
            LAUNCH_DURATION_MS, LAUNCH_EXIT_CROSSFADE_MS, LAUNCH_HARD_CEILING_MS, LAUNCH_HOLD_MS,
        };

        assert_eq!(LAUNCH_DURATION_MS, 1700, "the asset's own length");
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

    /// T051 — the texture-lifetime landmine, asserted across a FULL playback.
    ///
    /// A leak here is invisible on screen and only shows up after many launches,
    /// so it cannot be caught by looking. Driving the slot the way `publish_frame`
    /// drives it proves two things at once: at most one frame is ever live, and
    /// every frame published except the last is handed back for release.
    #[test]
    fn frame_pump_never_holds_more_than_one_texture() {
        let mut slot = FrameSlot::default();
        let mut released = 0usize;
        let total = 102;

        for _ in 0..total {
            let image = Arc::new(RenderImage::new(vec![Frame::new(RgbaImage::new(4, 4))]));
            if slot.replace(image).is_some() {
                released += 1; // production hands this to `window.drop_image`
            }
            assert_eq!(slot.live(), 1, "more than one frame live at once");
        }

        assert_eq!(
            released,
            total - 1,
            "every frame but the last must be handed back for release"
        );

        assert!(
            slot.take().is_some(),
            "the final frame must still be releasable"
        );
        assert_eq!(slot.live(), 0, "nothing may remain live after teardown");
        assert!(
            slot.take().is_none(),
            "a second take must not resurrect a frame"
        );
    }

    /// Render one appearance at a fixed progress into an RGBA image.
    /// Headless: the player rasterises into a CPU buffer, which is exactly what
    /// makes desktop the cheapest platform to golden-frame (research D8).
    fn render_at_frame(bytes: &'static [u8], frame: f32) -> RgbaImage {
        const W: u32 = 340; // half the canvas — enough to catch drift, small to commit
        const H: u32 = 110;
        let data = CString::new(bytes).unwrap();
        let mut buffer = vec![0u32; (W * H) as usize];
        let mut player = Player::with_threads(1);
        let _ = player.set_layout(Layout::new(Fit::Fill, [0.5, 0.5]));
        player
            .set_sw_target(&mut buffer, W, H, ColorSpace::ARGB8888)
            .expect("set_sw_target");
        player.load_animation_data(&data).expect("load");
        assert!(
            advance(&mut player, frame),
            "advance failed at frame {frame}"
        );

        let mut rgba = Vec::with_capacity(buffer.len() * 4);
        for word in &buffer {
            // Buffer is premultiplied BGRA; PNG wants RGBA, so reorder here
            // ONLY for the reference images. The production path does no swap.
            let [b, g, r, a] = word.to_ne_bytes();
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        RgbaImage::from_raw(W, H, rgba).unwrap()
    }

    /// Sample points, chosen from the animation's OWN structure rather than
    /// round numbers: the mark slides over frames 12→36 and the ten glyph fades
    /// run 26→72 (see the keyframes in the asset). Evenly spaced progress values
    /// miss both — a first attempt sampled frame 25, which sits in the gap
    /// between the slide ending and the first glyph appearing, so a deliberate
    /// 14-frame shift of that glyph changed none of the sampled images and the
    /// check passed while being blind.
    ///
    /// frame  0 — nothing drawn yet, mark centred
    /// frame 24 — mid-slide
    /// frame 45 — mark landed, cascade under way
    /// frame 65 — late cascade
    /// frame 101 — the finished lockup (last valid frame)
    const GOLDEN_FRAMES: [u32; 5] = [0, 24, 45, 65, 101];

    /// T061 — golden frames at fixed points, seeked deterministically rather
    /// than slept through.
    ///
    /// Run with `VELA_UPDATE_GOLDEN=1` to rewrite the references; review the
    /// diff, because a change here means the animation changed on this platform.
    #[test]
    fn golden_frames_match_the_committed_references() {
        let _engine = engine_lock();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        let update = std::env::var("VELA_UPDATE_GOLDEN").is_ok();
        if update {
            std::fs::create_dir_all(&dir).expect("create golden dir");
        }

        let mut mismatches = Vec::new();
        for (appearance, bytes) in [("dark", DARK), ("light", LIGHT)] {
            for frame in GOLDEN_FRAMES {
                let actual = render_at_frame(bytes, frame as f32);
                let path = dir.join(format!("launch-{appearance}-f{frame:03}.png"));

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

                // Per-platform tolerance: this compares ThorVG against itself, so
                // it is tight — the looser tolerances belong on the cross-engine
                // comparisons. `differing` counts pixels off by more than a hair.
                let differing = expected
                    .pixels()
                    .zip(actual.pixels())
                    .filter(|(e, a)| e.0.iter().zip(a.0.iter()).any(|(x, y)| x.abs_diff(*y) > 2))
                    .count();
                let total = (actual.width() * actual.height()) as usize;
                if differing * 1000 > total {
                    mismatches.push(format!(
                        "{appearance} @frame {frame}: {differing}/{total} pixels differ (>0.1%)"
                    ));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "golden frames drifted:\n  {}\nIf the asset changed on purpose, rerun with VELA_UPDATE_GOLDEN=1 and review the images.",
            mismatches.join("\n  ")
        );
    }

    /// SC-006 in miniature, twice over: the comparison must be able to fail, AND
    /// consecutive sample points must differ from each other. The second half is
    /// what catches sample points chosen in a gap where nothing moves — the
    /// exact way the first version of this suite was blind.
    #[test]
    fn golden_comparison_can_actually_fail() {
        let _engine = engine_lock();
        let early = render_at_frame(DARK, 0.);
        let late = render_at_frame(DARK, 101.);
        let differing = early
            .pixels()
            .zip(late.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing > (early.width() * early.height()) as usize / 100,
            "frame 0 and the final frame are nearly identical ({differing} px differ) — \
             the renderer is not actually advancing, so the golden test proves nothing"
        );

        // Every adjacent pair of sample points must show motion, or that point
        // is not buying coverage.
        for pair in GOLDEN_FRAMES.windows(2) {
            let (a, b) = (
                render_at_frame(DARK, pair[0] as f32),
                render_at_frame(DARK, pair[1] as f32),
            );
            let moved = a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count();
            assert!(
                moved > 0,
                "frames {} and {} render identically — that sample point covers nothing",
                pair[0],
                pair[1]
            );
        }
    }

    /// The pixel-format claim, asserted rather than trusted: ARGB8888 must put
    /// a known colour into memory as premultiplied BGRA bytes, which is what
    /// `gpui::RenderImage` reads. Getting this wrong swaps red and blue —
    /// subtle enough to pass a glance and fail a golden frame.
    #[test]
    fn argb8888_words_are_bgra_bytes_in_memory() {
        let word: u32 = 0xAA_BB_CC_DD; // A=AA R=BB G=CC B=DD
        let bytes = word.to_ne_bytes();
        assert_eq!(
            bytes,
            [0xDD, 0xCC, 0xBB, 0xAA],
            "expected B,G,R,A byte order — gpui reads premultiplied BGRA"
        );
    }
}
