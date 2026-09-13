//! The scanner's live half: a camera, on the platforms that have one here.
//!
//! macOS only, and that is a grading rather than a gap. Phase 1's file path
//! (`executor::qr`) is the scanner's FLOOR — it works on all three desktops —
//! so a platform without a capture backend loses the preview and keeps the
//! feature. `nokhwa` is compiled with `input-avfoundation` alone for the same
//! reason: naming the v4l or Media Foundation backends would drag those
//! systems' headers into targets that do not build them.
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
    /// The camera could not be used. The reason is logged, not shown: the
    /// screen's answer is the same for all of them, and "AVFoundation error
    /// -11852" is not a sentence for a person.
    pub failed: bool,
}

/// A running capture. Dropping it stops the thread.
pub struct Session {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Session {
    /// The newest frame and whether the camera is usable, cloned out.
    #[must_use]
    pub fn snapshot(&self) -> (Option<RgbaImage>, bool) {
        match self.shared.lock() {
            Ok(shared) => (shared.frame.clone(), shared.failed),
            Err(_) => (None, true),
        }
    }

    /// The payload, if a frame has decoded one — taken, so a scan fires once.
    #[must_use]
    pub fn take_payload(&self) -> Option<String> {
        self.shared.lock().ok()?.payload.take()
    }
}

/// Open the default camera and start reading it.
///
/// Returns immediately; the first frame arrives when the sensor is ready
/// (which on macOS is after the permission prompt, if this is the first ask).
#[must_use]
pub fn start() -> Session {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let stop = Arc::new(AtomicBool::new(false));
    spawn_capture(&shared, &stop);
    Session { shared, stop }
}

#[cfg(target_os = "macos")]
fn spawn_capture(shared: &Arc<Mutex<Shared>>, stop: &Arc<AtomicBool>) {
    let shared = Arc::clone(shared);
    let stop = Arc::clone(stop);
    std::thread::spawn(move || {
        use nokhwa::pixel_format::RgbFormat;
        use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

        let fail = |shared: &Arc<Mutex<Shared>>, why: &str| {
            eprintln!("[vela-wallet] camera: {why}");
            if let Ok(mut shared) = shared.lock() {
                shared.failed = true;
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
            fail(&shared, "permission was not granted");
            return;
        }

        let format =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let Ok(mut camera) = nokhwa::Camera::new(CameraIndex::Index(0), format) else {
            fail(&shared, "no camera could be opened");
            return;
        };
        if camera.open_stream().is_err() {
            fail(&shared, "the camera would not start streaming");
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
        shared.failed = true;
    }
}
