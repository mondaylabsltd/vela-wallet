//! The web's motion tokens, on the desktop (N-ALL premium motion).
//!
//! `docs/design-tokens.json` → the web's `tokens.css`: the third column opens
//! in `--motion-panel-in` (240 ms) sliding in from its edge as it fades;
//! a card over a scrim (the web's signing/prompt card at desktop width)
//! rises `--space-5xl` in `--motion-sheet-in` (220 ms) while the scrim fades;
//! a toast drops in over the same 220 ms. All `ease-out` — CSS's
//! `cubic-bezier(0, 0, 0.58, 1)`, solved here rather than approximated, so a
//! frame lands where the browser's would.
//!
//! **Reduce motion is the system's**, read from the OS ([`sync_system`]:
//! macOS's Reduce Motion, Windows' "Show animations in Windows", GNOME's
//! `enable-animations`) into gpui's `reduce_motion`, which nothing set before
//! — every `cx.reduce_motion()` in this app read a constant `false`. Under it
//! nothing travels: a column or card degrades to `--motion-crossfade`
//! (150 ms opacity), as the web's `prefers-reduced-motion` rules do.
//!
//! A layout probe (`VELA_LAYOUT_PROBE`) measures where things REST, so motion
//! is off while one runs — a frame mid-slide is not a position.

use std::time::Duration;

use gpui::{
    AnimationExt as _, AnyElement, App, Div, ElementId, IntoElement, ParentElement as _, Stateful,
    Styled as _, px,
};

/// `--motion-panel-in`: the third column opening.
pub const PANEL_IN: Duration = Duration::from_millis(crate::theme::CONTACTS_MOTION_PANEL_OPEN_MS);
/// `--motion-sheet-in`: a card or a toast arriving.
pub const SHEET_IN: Duration = Duration::from_millis(220);
/// `--motion-crossfade`: what every movement degrades to under Reduce Motion.
pub const CROSSFADE: Duration = Duration::from_millis(crate::theme::CONTACTS_MOTION_CROSSFADE_MS);
/// `--space-5xl`: how far a card rises as it arrives.
pub const CARD_RISE: f32 = 48.;
/// `--space-lg`: how far a toast drops as it arrives.
pub const TOAST_DROP: f32 = 12.;

/// CSS `ease-out`: `cubic-bezier(0, 0, 0.58, 1)` at time `t` in `0..=1`.
#[must_use]
pub fn ease_out(t: f32) -> f32 {
    cubic_bezier(0., 0., 0.58, 1., t)
}

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function: the curve's `y` at
/// the `x` that equals `t`, `x` solved by Newton's method with a bisection
/// fallback (the browser's own approach, WebKit's `UnitBezier`).
#[must_use]
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    let (cx, cy) = (3. * x1, 3. * y1);
    let (bx, by) = (3. * (x2 - x1) - cx, 3. * (y2 - y1) - cy);
    let (ax, ay) = (1. - cx - bx, 1. - cy - by);
    let x_at = |s: f32| ((ax * s + bx) * s + cx) * s;
    let dx_at = |s: f32| (3. * ax * s + 2. * bx) * s + cx;
    let y_at = |s: f32| ((ay * s + by) * s + cy) * s;
    let mut s = t;
    for _ in 0..8 {
        let err = x_at(s) - t;
        if err.abs() < 1e-6 {
            return y_at(s);
        }
        let d = dx_at(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= err / d;
    }
    let (mut lo, mut hi) = (0_f32, 1_f32);
    s = t;
    for _ in 0..32 {
        let x = x_at(s);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.;
    }
    y_at(s)
}

/// The system's answer, as [`sync_system`] last read it — for the drawing
/// code that has a window but no `App` (a dialog).
static REDUCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether movement is off: the system asked for less motion, or a layout
/// probe is measuring.
#[must_use]
pub fn reduced() -> bool {
    REDUCE.load(std::sync::atomic::Ordering::Relaxed) || crate::dev_probe::on()
}

/// Read the system's Reduce Motion into gpui. At launch and whenever the
/// window comes back to the front (a setting changed meanwhile is in force by
/// the next movement). `VELA_REDUCE_MOTION=1` pins it in a developer build.
pub fn sync_system(cx: &mut App) {
    let pinned = crate::dev_env::var!("VELA_REDUCE_MOTION").map(|value| value == "1");
    let reduce = pinned.unwrap_or_else(system_reduces_motion);
    REDUCE.store(reduce, std::sync::atomic::Ordering::Relaxed);
    cx.set_reduce_motion(reduce);
}

#[cfg(target_os = "macos")]
fn system_reduces_motion() -> bool {
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    // SAFETY: `+[NSWorkspace sharedWorkspace]` never returns nil, and
    // `accessibilityDisplayShouldReduceMotion` is a plain BOOL getter
    // (macOS 10.12+; this app's floor is 11).
    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let reduce: Bool = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce.as_bool()
    }
}

#[cfg(windows)]
fn system_reduces_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut animate: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL into `animate`,
    // which lives for the call.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            std::ptr::from_mut(&mut animate).cast(),
            0,
        )
    };
    // "Show animations in Windows" off is the system's reduce-motion.
    read != 0 && animate == 0
}

#[cfg(target_os = "linux")]
fn system_reduces_motion() -> bool {
    // GNOME (and the desktops that follow its schema): animations switched
    // off. No `gsettings`, no schema: the default, which is motion.
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|value| value.trim() == "false")
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn system_reduces_motion() -> bool {
    false
}

/// The third column arriving: from its own edge (`translateX(100%)` →
/// `0`) as it fades in, over [`PANEL_IN`]; a fade alone under Reduce Motion.
/// The returned element clips the slide, and takes the column's place.
pub fn panel_in(column: Div, width: f32, reduce: bool) -> Div {
    let moving = if reduce {
        column
            .with_animation(
                "third-column-in",
                gpui::Animation::new(CROSSFADE).with_easing(ease_out),
                |el, t| el.opacity(t),
            )
            .into_any_element()
    } else {
        column
            .with_animation(
                "third-column-in",
                gpui::Animation::new(PANEL_IN).with_easing(ease_out),
                move |el, t| el.opacity(t).left(px(width * (1. - t))),
            )
            .into_any_element()
    };
    gpui::div()
        .h_full()
        .flex_none()
        .overflow_hidden()
        .child(moving)
}

/// A dialog's card rising into place over [`SHEET_IN`] (the web's
/// `card-in`); a fade under Reduce Motion.
pub fn card_in(card: Stateful<Div>, id: impl Into<ElementId>, reduce: bool) -> AnyElement {
    if reduce {
        return card
            .with_animation(
                id,
                gpui::Animation::new(CROSSFADE).with_easing(ease_out),
                |el, t| el.opacity(t),
            )
            .into_any_element();
    }
    card.relative()
        .with_animation(
            id,
            gpui::Animation::new(SHEET_IN).with_easing(ease_out),
            |el, t| el.opacity(t).top(px(CARD_RISE * (1. - t))),
        )
        .into_any_element()
}

/// The layer behind a card fading in with it (the web's `scrim-in`, which
/// keeps its fade under Reduce Motion as an opacity change).
pub fn scrim_in(layer: Div, id: impl Into<ElementId>, reduce: bool) -> AnyElement {
    let duration = if reduce { CROSSFADE } else { SHEET_IN };
    layer
        .with_animation(
            id,
            gpui::Animation::new(duration).with_easing(ease_out),
            |el, t| el.opacity(t),
        )
        .into_any_element()
}

/// A toast dropping in (the web's `toast-in`: [`TOAST_DROP`] over
/// [`SHEET_IN`]); a fade under Reduce Motion.
pub fn toast_in(pill: Div, id: impl Into<ElementId>, reduce: bool) -> AnyElement {
    if reduce {
        return pill
            .with_animation(
                id,
                gpui::Animation::new(CROSSFADE).with_easing(ease_out),
                |el, t| el.opacity(t),
            )
            .into_any_element();
    }
    pill.with_animation(
        id,
        gpui::Animation::new(SHEET_IN).with_easing(ease_out),
        |el, t| el.opacity(t).top(px((t - 1.) * TOAST_DROP)),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_is_the_browsers_curve() {
        // The ends are the ends.
        assert!(ease_out(0.).abs() < 1e-5);
        assert!((ease_out(1.) - 1.).abs() < 1e-5);
        // cubic-bezier(0,0,.58,1) at x = .5 is y ≈ .68464 (bisected independently).
        assert!((ease_out(0.5) - 0.684_64).abs() < 1e-4, "{}", ease_out(0.5));
        // Out: fast first, never past the end, never backwards.
        let mut last = 0.;
        for step in 1..=100 {
            let y = ease_out(step as f32 / 100.);
            assert!(y >= last && y <= 1.0001, "monotone at {step}: {y}");
            last = y;
        }
        assert!(ease_out(0.25) > 0.25, "an ease-out leads");
        // Linear is linear.
        assert!((cubic_bezier(0., 0., 1., 1., 0.3) - 0.3).abs() < 1e-4);
    }

    #[test]
    fn the_timings_are_the_web_tokens() {
        // The web's generated stylesheet carries both the design tokens and
        // its own desktop additions (`--motion-panel-in`, `--motion-crossfade`).
        let css = include_str!("../../../../app-web/vela-wallet/src/lib/tokens/tokens.css");
        let token = |name: &str| -> f32 {
            let at = css
                .find(&format!("--{name}:"))
                .unwrap_or_else(|| unreachable!("--{name} is in tokens.css"));
            css[at + name.len() + 3..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
                .parse()
                .unwrap_or_default()
        };
        let ms = |d: Duration| d.as_millis() as f32;
        assert_eq!(token("motion-sheet-in"), ms(SHEET_IN));
        assert_eq!(token("motion-panel-in"), ms(PANEL_IN));
        assert_eq!(token("motion-crossfade"), ms(CROSSFADE));
        assert_eq!(token("space-5xl"), CARD_RISE);
        assert_eq!(token("space-lg"), TOAST_DROP);
    }
}
