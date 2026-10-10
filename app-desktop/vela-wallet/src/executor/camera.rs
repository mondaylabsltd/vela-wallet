//! The scanner's live half: a camera, on the platforms that have one here.
//!
//! macOS only, and that is a grading rather than a gap. Phase 1's file path
//! (`executor::qr`) is the scanner's FLOOR — it works on all three desktops —
//! so a platform without a capture backend loses the preview and keeps the
//! feature. `nokhwa` is compiled with `input-avfoundation` alone for the same
//! reason: naming the v4l or Media Foundation backends would drag those
//! systems' headers into targets that do not build them.
//!
//! ## A fixture session never opens a camera (PR 3 note 7)
//!
//! The gallery, the design pages and any session a developer pin put its
//! flow stack up in (`VELA_FLOW=DS1`) draw the scanner without anybody having
//! asked to scan. A sweep of those states once turned the real camera on and
//! captured a frame of the person at the machine. So where the frames come
//! from is decided before anything starts ([`Source::for_session`]), and a
//! [`Source::Fixture`] session never reaches the capture backend at all — no
//! permission ask, no device, no thread: its one frame is DRAWN
//! ([`fixture_frame`]), a sample code on a card, and it never reports a
//! payload. [`start`] is the only way a capture begins, and it cannot begin
//! one without being handed [`Source::Camera`].
//!
//! ## Everything here fails soft
//!
//! No camera, a camera another app holds, a person who said no to the
//! permission prompt, a driver that hands back a format this build cannot
//! decode — all of them land in the same place: `failed`, the viewfinder keeps
//! its placeholder, and "from a picture" is still right there on the toolbar.
//! A wallet that dies because a webcam is busy is a worse wallet than one that
//! quietly offers the other door.
//!
//! ## Why a thread and not an async task
//!
//! `Camera::frame` blocks for as long as the sensor takes. On gpui's
//! foreground that is the window not drawing; on its background executor it
//! would hold one of a small pool of threads for the life of the scan. So the
//! capture owns a thread of its own, publishes into a mutex, and the screen
//! reads whatever is latest — the same shape `wallet::money`'s ceremony uses.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use image::RgbaImage;

/// What the screen reads. Never blocks the capture: the lock is held for the
/// length of one move.
#[derive(Default)]
pub struct Shared {
    /// The newest frame, in the RGBA the renderer wants.
    pub frame: Option<RgbaImage>,
    /// The first payload any frame decoded to. Taken once by the screen.
    pub payload: Option<String>,
    /// Why the camera could not be used, when it could not (078 F-02). The
    /// driver's own words are logged, not shown — "AVFoundation error -11852"
    /// is not a sentence for a person — but WHICH of three things went wrong
    /// is: each has a different thing to do about it.
    pub failure: Option<CameraFailure>,
}

/// The three ways a camera is not there, as the web's scanner tells them
/// apart (`scanner.svelte.ts` `classify`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    not(target_os = "macos"),
    expect(
        dead_code,
        reason = "only the macOS capture can be refused or busy; elsewhere it is Absent"
    )
)]
pub enum CameraFailure {
    /// The person said no (or said no once and was never asked again).
    Denied,
    /// No camera to open — including every desktop this build has no capture
    /// backend for.
    Absent,
    /// A camera that would not start: another app has it, or the driver
    /// refused.
    Unavailable,
}

/// Where a scanner's frames come from. Chosen once, before anything starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The machine's own camera: a person's own scanner, in their own session.
    Camera,
    /// A drawn frame ([`fixture_frame`]). No device is opened.
    Fixture,
}

impl Source {
    /// The rule (PR 3 note 7): the real camera is for a signed-in person's
    /// own scanner and nothing else. The gallery (`gallery`), a design page
    /// (nobody is signed in: everything it draws is a fixture) and a session
    /// whose flow stack a developer pin put up (`flow_pinned`, `VELA_FLOW`) —
    /// where the scanner is on screen with no click behind it — draw the
    /// fixture frame.
    #[must_use]
    pub fn for_session(gallery: bool, signed_in: bool, flow_pinned: bool) -> Self {
        if gallery || !signed_in || flow_pinned {
            Self::Fixture
        } else {
            Self::Camera
        }
    }
}

/// A running capture. Dropping it stops the thread.
pub struct Session {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    source: Source,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Session {
    /// The newest frame and why the camera is not usable, if it is not.
    #[must_use]
    pub fn snapshot(&self) -> (Option<RgbaImage>, Option<CameraFailure>) {
        match self.shared.lock() {
            Ok(shared) => (shared.frame.clone(), shared.failure),
            Err(_) => (None, Some(CameraFailure::Unavailable)),
        }
    }

    /// The payload, if a frame has decoded one — taken, so a scan fires once.
    #[must_use]
    pub fn take_payload(&self) -> Option<String> {
        self.shared.lock().ok()?.payload.take()
    }

    /// Is this the drawn frame rather than a camera? Its one frame never
    /// changes, so the screen publishes it once and asks for no repaints.
    #[must_use]
    pub fn is_fixture(&self) -> bool {
        self.source == Source::Fixture
    }
}

/// Start a scanner's frames: the default camera, or — for a fixture session
/// — the drawn frame, with no camera opened at all.
///
/// Returns immediately; a camera's first frame arrives when the sensor is
/// ready (which on macOS is after the permission prompt, if this is the first
/// ask). The fixture's frame is there at once.
#[must_use]
pub fn start(source: Source) -> Session {
    start_with(source, spawn_capture)
}

/// [`start`], with the capture backend named — so a test can prove a fixture
/// session never reaches it without a camera ever being opened by a test.
fn start_with(
    source: Source,
    capture: impl FnOnce(&Arc<Mutex<Shared>>, &Arc<AtomicBool>),
) -> Session {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let stop = Arc::new(AtomicBool::new(false));
    // Said in the log either way, so a sweep's logs can be searched for a
    // camera that was opened where none should have been.
    match source {
        Source::Camera => {
            crate::diag::vlog!("camera", "opening the default camera");
            capture(&shared, &stop);
        }
        // Nothing is opened, asked for or spawned: the frame is drawn.
        Source::Fixture => {
            crate::diag::vlog!("camera", "fixture frame — no camera is opened");
            if let Ok(mut shared) = shared.lock() {
                shared.frame = Some(fixture_frame());
            }
        }
    }
    Session {
        shared,
        stop,
        source,
    }
}

/// What the fixture frame's sample code says: the design fixtures' own
/// wallet, as a payment link — a code like the one a person would hold up.
pub const FIXTURE_PAYLOAD: &str =
    concat!("ethereum:", "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c");

/// The fixture session's one frame: a sample code on a paper card on a desk,
/// in the viewfinder's own 3:2. Drawn here, pixel by pixel — nothing in it
/// was ever in front of a lens. Deterministic, so a screenshot of the
/// gallery's scanner is the same picture every time.
#[must_use]
pub fn fixture_frame() -> RgbaImage {
    const WIDTH: u32 = 960;
    const HEIGHT: u32 = 640;
    /// One module of the code, in frame pixels.
    const MODULE: u32 = 8;
    /// The quiet zone the format asks for, in modules.
    const QUIET: u32 = 4;
    // A mid tone: the viewfinder's corner brackets are the theme's ink —
    // dark in the light theme, light in the dark one — and read over it in
    // both.
    const DESK: [u8; 4] = [0x8A, 0x85, 0x7D, 0xFF];
    const PAPER: [u8; 4] = [0xF6, 0xF3, 0xEE, 0xFF];
    const INK: [u8; 4] = [0x1C, 0x19, 0x17, 0xFF];

    let mut frame = RgbaImage::from_pixel(WIDTH, HEIGHT, image::Rgba(DESK));
    // A soft pool of light toward the middle, so the desk is not a flat fill.
    for (x, y, pixel) in frame.enumerate_pixels_mut() {
        let dx = (f64::from(x) - f64::from(WIDTH) / 2.) / f64::from(WIDTH);
        let dy = (f64::from(y) - f64::from(HEIGHT) / 2.) / f64::from(HEIGHT);
        let lift = (1. - (dx * dx + dy * dy).sqrt() * 1.6).clamp(0., 1.) * 18.;
        for channel in pixel.0.iter_mut().take(3) {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::allow_attributes,
                reason = "0..=18 added to a channel, clamped to a byte"
            )]
            let lifted = (f64::from(*channel) + lift).min(255.) as u8;
            *channel = lifted;
        }
    }
    let Ok(code) = qrcode::QrCode::new(FIXTURE_PAYLOAD.as_bytes()) else {
        // The payload is a constant that encodes; a desk with no card is
        // still a frame, and still not a camera.
        return frame;
    };
    let modules = u32::try_from(code.width()).unwrap_or(0);
    let colors = code.to_colors();
    let side = (modules + QUIET * 2) * MODULE;
    let (left, top) = (
        WIDTH.saturating_sub(side) / 2,
        HEIGHT.saturating_sub(side) / 2,
    );
    for y in 0..side.min(HEIGHT) {
        for x in 0..side.min(WIDTH) {
            frame.put_pixel(left + x, top + y, image::Rgba(PAPER));
        }
    }
    for my in 0..modules {
        for mx in 0..modules {
            let dark = colors
                .get((my * modules + mx) as usize)
                .is_some_and(|color| *color == qrcode::Color::Dark);
            if !dark {
                continue;
            }
            for dy in 0..MODULE {
                for dx in 0..MODULE {
                    let x = left + (mx + QUIET) * MODULE + dx;
                    let y = top + (my + QUIET) * MODULE + dy;
                    if x < WIDTH && y < HEIGHT {
                        frame.put_pixel(x, y, image::Rgba(INK));
                    }
                }
            }
        }
    }
    frame
}

#[cfg(target_os = "macos")]
fn spawn_capture(shared: &Arc<Mutex<Shared>>, stop: &Arc<AtomicBool>) {
    let shared = Arc::clone(shared);
    let stop = Arc::clone(stop);
    std::thread::spawn(move || {
        use nokhwa::pixel_format::RgbFormat;
        use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

        let fail = |shared: &Arc<Mutex<Shared>>, failure: CameraFailure, why: &str| {
            eprintln!("[vela-wallet] camera: {why}");
            if let Ok(mut shared) = shared.lock() {
                shared.failure = Some(failure);
            }
        };

        // macOS gates capture behind TCC, and the ask has to happen before the
        // device is opened. Blocking until the person answers is deliberate:
        // this thread has nothing else to do, and starting a stream against an
        // unanswered prompt is how the first frame comes back black.
        let granted = Arc::new(AtomicBool::new(false));
        let answered = Arc::new(AtomicBool::new(false));
        {
            let granted = Arc::clone(&granted);
            let answered = Arc::clone(&answered);
            nokhwa::nokhwa_initialize(move |ok| {
                granted.store(ok, Ordering::SeqCst);
                answered.store(true, Ordering::SeqCst);
            });
        }
        for _ in 0..300 {
            if answered.load(Ordering::SeqCst) || stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
        if !granted.load(Ordering::SeqCst) {
            fail(&shared, CameraFailure::Denied, "permission was not granted");
            return;
        }

        let format =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let Ok(mut camera) = nokhwa::Camera::new(CameraIndex::Index(0), format) else {
            fail(&shared, CameraFailure::Absent, "no camera could be opened");
            return;
        };
        if camera.open_stream().is_err() {
            fail(
                &shared,
                CameraFailure::Unavailable,
                "the camera would not start streaming",
            );
            return;
        }

        while !stop.load(Ordering::SeqCst) {
            let Ok(buffer) = camera.frame() else {
                // One dropped frame is not a failure; a camera that has gone
                // away answers this way every time, and the loop below ends on
                // the stop flag either way.
                std::thread::sleep(std::time::Duration::from_millis(30));
                continue;
            };
            let Ok(rgb) = buffer.decode_image::<RgbFormat>() else {
                continue;
            };
            let payload = crate::executor::qr::decode_luma(&rgb);
            let rgba = image::DynamicImage::ImageRgb8(rgb).to_rgba8();
            if let Ok(mut shared) = shared.lock() {
                shared.frame = Some(rgba);
                if let Some(text) = payload {
                    if shared.payload.is_none() {
                        shared.payload = Some(text);
                    }
                }
            }
        }
        let _ = camera.stop_stream();
    });
}

/// Every other desktop: no capture backend compiled in, so the session is born
/// failed and the file path is the whole scanner. Written as its own function
/// rather than a `cfg` inside the thread so the Windows and Linux builds carry
/// no camera code at all.
#[cfg(not(target_os = "macos"))]
fn spawn_capture(shared: &Arc<Mutex<Shared>>, _stop: &Arc<AtomicBool>) {
    if let Ok(mut shared) = shared.lock() {
        shared.failure = Some(CameraFailure::Absent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PR 3 note 7: the real camera is for a signed-in person's own scanner.
    /// The gallery, a design page and any session a developer pin put its
    /// flow stack up in draw the fixture frame.
    #[test]
    fn only_a_signed_in_persons_own_scanner_gets_the_camera() {
        // (gallery, signed in, flow pinned)
        assert_eq!(Source::for_session(false, true, false), Source::Camera);
        // The gallery — `VELA_PAGE=gallery`, with or without `VELA_FLOW=DS1`.
        assert_eq!(Source::for_session(true, false, false), Source::Fixture);
        assert_eq!(Source::for_session(true, false, true), Source::Fixture);
        // A design page: `VELA_PAGE=wallet`, nobody signed in.
        assert_eq!(Source::for_session(false, false, false), Source::Fixture);
        assert_eq!(Source::for_session(false, false, true), Source::Fixture);
        // A real session with the scanner pinned up by `VELA_FLOW=DS1`: the
        // screenshot passes' own route, where nobody clicked "Scan".
        assert_eq!(Source::for_session(false, true, true), Source::Fixture);
        // Every other combination with a gallery in it.
        assert_eq!(Source::for_session(true, true, false), Source::Fixture);
        assert_eq!(Source::for_session(true, true, true), Source::Fixture);
    }

    /// A fixture session never reaches the capture backend: no permission
    /// ask, no device, no thread. Proven with the backend replaced by a probe
    /// — so this test opens no camera either — which a camera session does
    /// reach, once.
    #[test]
    fn a_fixture_session_never_reaches_the_capture_backend() {
        let reached = std::sync::atomic::AtomicUsize::new(0);
        let probe = |_: &Arc<Mutex<Shared>>, _: &Arc<AtomicBool>| {
            reached.fetch_add(1, Ordering::SeqCst);
        };

        let fixture = start_with(Source::Fixture, probe);
        assert_eq!(reached.load(Ordering::SeqCst), 0, "the camera was started");
        assert!(fixture.is_fixture());
        let (frame, failure) = fixture.snapshot();
        assert!(frame.is_some(), "the drawn frame is there at once");
        assert_eq!(failure, None, "a fixture is not a camera that failed");
        assert_eq!(fixture.take_payload(), None, "nothing was scanned");

        // The probe does see a camera session — so the zero above means
        // something.
        let camera = start_with(Source::Camera, probe);
        assert_eq!(reached.load(Ordering::SeqCst), 1);
        assert!(!camera.is_fixture());

        // And `start` itself, as the screen calls it.
        let fixture = start(Source::Fixture);
        assert!(fixture.is_fixture() && fixture.snapshot().0.is_some());
    }

    /// The fixture frame is a picture of a code: the viewfinder's own 3:2,
    /// the same every time, and its sample reads back as the design
    /// fixtures' wallet — a real code, drawn rather than photographed.
    #[test]
    fn the_fixture_frame_is_a_drawn_sample_code() {
        let frame = fixture_frame();
        assert_eq!(frame.width() * 2, frame.height() * 3, "3:2");
        assert_eq!(frame, fixture_frame(), "deterministic");
        let rgb = image::DynamicImage::ImageRgba8(frame).to_rgb8();
        assert_eq!(
            crate::executor::qr::decode_luma(&rgb).as_deref(),
            Some(FIXTURE_PAYLOAD)
        );
        assert!(FIXTURE_PAYLOAD.ends_with(crate::wallet::fixtures::ADDRESS_FULL));
    }
}
