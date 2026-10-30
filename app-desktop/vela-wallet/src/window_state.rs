//! The window comes back where the person left it (N-D3 polish): its size,
//! its place on the display it was on, whether it was zoomed, and the section
//! it showed (Wallet, Contacts, Explore, Settings).
//!
//! One record in the store, `vela.desktop.window` — a device preference like
//! `vela.theme`, so an erase removes it with everything else ("vela." keys
//! are erasable by default, `vela_core::storage_catalog`).
//!
//! **Where it goes back to is checked, never trusted.** A display can be
//! unplugged between two launches, a resolution can shrink, and a record can
//! be from a machine with a bigger screen: [`placement`] keeps the window on a
//! display that exists now, no bigger than it and with its title strip on it,
//! and otherwise centres it on the primary display — the first-launch
//! behaviour. Positions are kept RELATIVE to the display (gpui's
//! `Window::bounds` is display-relative and `WindowOptions::display_id` places
//! by it), which sidesteps the macOS quirk that every display reports origin
//! `(0, 0)` (README, "Known gpui quirks").
//!
//! Full screen comes back as the window it was before, not as a new Space:
//! a second display's full screen has no titlebar reveal at this gpui pin
//! (README), and a window that opens into one and cannot be got out of is
//! worse than one that opens a click away from it.

use std::sync::Mutex;

use gpui::{App, Bounds, DisplayId, Pixels, Size, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

/// The store key.
pub const KEY: &str = "vela.desktop.window";

/// How much of a window must stay on its display, so it can still be
/// grabbed: its title strip, and this much of its width.
const KEEP_VISIBLE: f32 = 120.;

/// Writes wait this long after the last move or resize, so a drag is one
/// write, not one per frame.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Windowed,
    Maximized,
    /// Saved as such; restored as the window it was (see the module note).
    Fullscreen,
}

/// What is kept between launches.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    /// The display's UUID, as gpui reads it; `None` when it could not say.
    #[serde(default)]
    pub display: Option<String>,
    /// Display-relative, in logical pixels: the window's bounds when it is
    /// windowed, its restore bounds otherwise.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub mode: Mode,
    /// `wallet` / `contacts` / `explore` / `settings`.
    #[serde(default)]
    pub section: Option<String>,
}

/// One display as [`placement`] sees it: its id, UUID and size.
pub struct Screen {
    pub id: DisplayId,
    pub uuid: Option<String>,
    pub size: Size<Pixels>,
}

/// Where the window opens: the display to open on (`None` = the primary)
/// and its bounds. `None` when nothing usable was saved — the caller centres
/// the window as on a first launch.
#[must_use]
pub fn placement(
    saved: Option<&Saved>,
    screens: &[Screen],
    min: Size<Pixels>,
) -> Option<(Option<DisplayId>, WindowBounds)> {
    let saved = saved?;
    if !(saved.width.is_finite()
        && saved.height.is_finite()
        && saved.x.is_finite()
        && saved.y.is_finite())
    {
        return None;
    }
    // The display it was on, if it is still here; otherwise none of them —
    // a position on a display that is gone means nothing on another one.
    let screen = saved
        .display
        .as_deref()
        .and_then(|uuid| screens.iter().find(|s| s.uuid.as_deref() == Some(uuid)))?;
    let (dw, dh) = (f32::from(screen.size.width), f32::from(screen.size.height));
    let width = saved.width.max(f32::from(min.width)).min(dw);
    let height = saved.height.max(f32::from(min.height)).min(dh);
    // Its title strip on the display, and enough of it to grab.
    let x = saved
        .x
        .clamp(KEEP_VISIBLE - width, (dw - KEEP_VISIBLE).max(0.));
    let y = saved.y.clamp(0., (dh - KEEP_VISIBLE).max(0.));
    let bounds = Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
    let bounds = match saved.mode {
        Mode::Maximized => WindowBounds::Maximized(bounds),
        Mode::Windowed | Mode::Fullscreen => WindowBounds::Windowed(bounds),
    };
    Some((Some(screen.id), bounds))
}

/// The record in the store, if there is a readable one.
#[must_use]
pub fn load() -> Option<Saved> {
    let value = crate::executor::storage::read_value(KEY).ok()??;
    serde_json::from_value(value).ok()
}

/// The displays this machine has now.
#[must_use]
pub fn screens(cx: &App) -> Vec<Screen> {
    cx.displays()
        .into_iter()
        .map(|display| Screen {
            id: display.id(),
            uuid: display.uuid().ok().map(|uuid| uuid.to_string()),
            size: display.bounds().size,
        })
        .collect()
}

/// The section saved last time — once per launch: the first wallet built
/// takes it, every later one (an account switch, a sign-in) starts on the
/// home. (`VELA_SECTION` still wins in a developer build; the caller reads
/// it first.)
pub fn take_launch_section() -> Option<String> {
    static TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if TAKEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    load().and_then(|saved| saved.section)
}

/// What is waiting to be written, and a counter so only the last of a burst
/// writes.
static PENDING: Mutex<(Option<Saved>, u64)> = Mutex::new((None, 0));

/// The record as it would be written now: the last one noted, or the stored
/// one, or nothing yet.
fn current() -> Saved {
    PENDING
        .lock()
        .ok()
        .and_then(|pending| pending.0.clone())
        .or_else(load)
        .unwrap_or_default()
}

/// Remember the window's bounds and mode. Called when the window moves or
/// resizes; the write lands once the window has been still for [`SETTLE`].
pub fn note_bounds(window: &Window, cx: &mut App) {
    let (mode, bounds) = match window.window_bounds() {
        WindowBounds::Windowed(bounds) => (Mode::Windowed, bounds),
        WindowBounds::Maximized(bounds) => (Mode::Maximized, bounds),
        WindowBounds::Fullscreen(bounds) => (Mode::Fullscreen, bounds),
    };
    let display = window
        .display(cx)
        .and_then(|display| display.uuid().ok())
        .map(|uuid| uuid.to_string());
    let next = Saved {
        display,
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
        mode,
        section: current().section,
    };
    stage(next, cx);
}

/// Remember the section on screen. Cheap to call every frame: only a change
/// is staged.
pub fn note_section(section: &str, cx: &mut App) {
    let mut next = current();
    if next.section.as_deref() == Some(section) {
        return;
    }
    // Nothing saved yet about the window itself: keep the section alone;
    // `placement` refuses a record with no display, so it opens centred.
    next.section = Some(section.to_owned());
    stage(next, cx);
}

fn stage(next: Saved, cx: &mut App) {
    let generation = {
        let Ok(mut pending) = PENDING.lock() else {
            return;
        };
        if pending.0.as_ref() == Some(&next) {
            return;
        }
        pending.0 = Some(next);
        pending.1 += 1;
        pending.1
    };
    cx.spawn(async move |cx| {
        cx.background_executor().timer(SETTLE).await;
        flush_if(generation);
    })
    .detach();
}

/// Write what is pending now (the window is closing, the app quitting).
pub fn flush() {
    let generation = PENDING.lock().map(|pending| pending.1).unwrap_or(0);
    flush_if(generation);
}

fn flush_if(generation: u64) {
    let saved = {
        let Ok(pending) = PENDING.lock() else {
            return;
        };
        if pending.1 != generation {
            return;
        }
        pending.0.clone()
    };
    if let Some(saved) = saved
        && let Ok(value) = serde_json::to_value(&saved)
        && let Err(error) = crate::executor::storage::write_value(KEY, value)
    {
        crate::diag::vlog!("window", "window state not saved: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: u64, uuid: &str, w: f32, h: f32) -> Screen {
        Screen {
            id: DisplayId::new(id),
            uuid: Some(uuid.to_owned()),
            size: size(px(w), px(h)),
        }
    }

    fn saved(display: &str, x: f32, y: f32, w: f32, h: f32, mode: Mode) -> Saved {
        Saved {
            display: Some(display.to_owned()),
            x,
            y,
            width: w,
            height: h,
            mode,
            section: Some("contacts".to_owned()),
        }
    }

    fn min() -> Size<Pixels> {
        size(px(1280.), px(800.))
    }

    fn windowed(placed: Option<(Option<DisplayId>, WindowBounds)>) -> (u64, [f32; 4]) {
        match placed {
            Some((Some(id), WindowBounds::Windowed(b))) => (
                u64::from(id),
                [
                    f32::from(b.origin.x),
                    f32::from(b.origin.y),
                    f32::from(b.size.width),
                    f32::from(b.size.height),
                ],
            ),
            other => unreachable!("not a windowed placement: {other:?}"),
        }
    }

    #[test]
    fn the_window_comes_back_where_it_was_on_the_display_it_was_on() {
        let screens = [screen(1, "A", 1728., 1117.), screen(2, "B", 2560., 1440.)];
        let placed = placement(
            Some(&saved("B", 300., 120., 1500., 900., Mode::Windowed)),
            &screens,
            min(),
        );
        assert_eq!(windowed(placed), (2, [300., 120., 1500., 900.]));
    }

    #[test]
    fn a_display_that_is_gone_opens_centred_like_a_first_launch() {
        let screens = [screen(1, "A", 1728., 1117.)];
        let gone = saved("B", 300., 120., 1500., 900., Mode::Windowed);
        assert!(placement(Some(&gone), &screens, min()).is_none());
        assert!(placement(None, &screens, min()).is_none());
        // A record from before a display was named: nothing to place by.
        let unnamed = Saved {
            display: None,
            ..gone
        };
        assert!(placement(Some(&unnamed), &screens, min()).is_none());
    }

    #[test]
    fn a_window_bigger_than_its_display_shrinks_and_never_below_the_design_size() {
        let screens = [screen(1, "A", 1440., 900.)];
        let placed = placement(
            Some(&saved("A", 0., 0., 2400., 1400., Mode::Windowed)),
            &screens,
            min(),
        );
        assert_eq!(windowed(placed), (1, [0., 0., 1440., 900.]));
        let tiny = placement(
            Some(&saved("A", 10., 10., 200., 100., Mode::Windowed)),
            &screens,
            min(),
        );
        assert_eq!(windowed(tiny), (1, [10., 10., 1280., 800.]));
    }

    #[test]
    fn a_window_dragged_off_the_edge_keeps_its_title_strip_on_screen() {
        let screens = [screen(1, "A", 1728., 1117.)];
        let placed = placement(
            Some(&saved("A", 5000., -300., 1280., 800., Mode::Windowed)),
            &screens,
            min(),
        );
        let (_, [x, y, ..]) = windowed(placed);
        assert_eq!(y, 0., "the title strip is never above the display");
        assert!(x <= 1728. - KEEP_VISIBLE, "enough of it stays to grab");
        let left = placement(
            Some(&saved("A", -5000., 50., 1280., 800., Mode::Windowed)),
            &screens,
            min(),
        );
        let (_, [x, ..]) = windowed(left);
        assert!(x + 1280. >= KEEP_VISIBLE);
    }

    #[test]
    fn zoomed_comes_back_zoomed_and_full_screen_comes_back_as_its_window() {
        let screens = [screen(1, "A", 1728., 1117.)];
        let zoomed = placement(
            Some(&saved("A", 0., 0., 1300., 820., Mode::Maximized)),
            &screens,
            min(),
        );
        assert!(matches!(zoomed, Some((_, WindowBounds::Maximized(_)))));
        let full = placement(
            Some(&saved("A", 40., 40., 1300., 820., Mode::Fullscreen)),
            &screens,
            min(),
        );
        assert_eq!(windowed(full), (1, [40., 40., 1300., 820.]));
    }

    #[test]
    fn a_damaged_record_is_no_record() {
        let screens = [screen(1, "A", 1728., 1117.)];
        let nan = saved("A", f32::NAN, 0., 1300., 820., Mode::Windowed);
        assert!(placement(Some(&nan), &screens, min()).is_none());
        // Unknown fields and a missing mode read; a wrong shape does not.
        let read: Saved = serde_json::from_str(
            r#"{"display":"A","x":1,"y":2,"width":1300,"height":820,"future":true}"#,
        )
        .unwrap_or_default();
        assert_eq!(read.mode, Mode::Windowed);
        assert!(serde_json::from_str::<Saved>(r#"{"x":"left"}"#).is_err());
    }
}
