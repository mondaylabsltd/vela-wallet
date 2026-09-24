//! A thin vertical scrollbar for an `overflow_y_scroll` column.
//!
//! gpui at this revision draws none: a column scrolls under the wheel and the
//! trackpad and shows nothing that says there is more below it. On a Mac that
//! passes, because the system hides scrollbars too; on Windows and Linux a
//! list cut off at the window's edge with no bar beside it reads as the whole
//! list. This draws the thumb only — where the column is, and how much of it
//! is on screen — from the column's own `ScrollHandle`. Dragging it is not
//! wired: the wheel, the trackpad and the keyboard already move the column.
//!
//! gpui notifies the view on every scroll, so the thumb is recomputed from
//! the handle each frame; the geometry it reads is the last layout's, which
//! is the frame the person is looking at.

use crate::theme::Theme;
use gpui::{Div, ParentElement as _, ScrollHandle, Styled as _, div, px};

/// Thinner than a platform bar: this marks position, it is not a control.
const WIDTH: f32 = 6.;
/// Inset from the column's right edge, so the thumb clears a row's hover tint.
const INSET: f32 = 3.;
/// The shortest a thumb gets, however long the column.
const MIN_THUMB: f32 = 28.;

/// The thumb for the column `handle` tracks, positioned over that column's
/// right edge — or `None` when everything fits and there is nothing to show.
///
/// The caller puts it in a `relative()` wrapper that is exactly the column's
/// box, next to the column itself.
#[must_use]
pub fn vertical_scrollbar(theme: &Theme, handle: &ScrollHandle) -> Option<Div> {
    let viewport = f32::from(handle.bounds().size.height);
    let max = f32::from(handle.max_offset().y);
    if viewport <= 0. || max <= 0.5 {
        return None;
    }
    // gpui's offset runs negative as the column scrolls down.
    let scrolled = (-f32::from(handle.offset().y)).clamp(0., max);
    let (thumb, top) = thumb_geometry(viewport, max, scrolled);
    Some(
        div()
            .absolute()
            .top_0()
            .right(px(INSET))
            .w(px(WIDTH))
            .h(px(viewport))
            .child(
                div()
                    .absolute()
                    .top(px(top))
                    .w_full()
                    .h(px(thumb))
                    .rounded(px(WIDTH / 2.))
                    .bg(theme.fg_subtle.opacity(0.45)),
            ),
    )
}

/// The thumb's length and its distance from the top of the track, which is
/// the column's visible height. `max` is how far the column can scroll and
/// `scrolled` how far it has.
fn thumb_geometry(viewport: f32, max: f32, scrolled: f32) -> (f32, f32) {
    let content = viewport + max;
    let thumb = (viewport * viewport / content).clamp(MIN_THUMB.min(viewport), viewport);
    let top = (viewport - thumb) * (scrolled / max);
    (thumb, top)
}

#[cfg(test)]
mod tests {
    use super::thumb_geometry;

    #[test]
    fn the_thumb_is_the_share_on_screen_and_travels_the_whole_track() {
        // Half the column on screen: a half-height thumb, top at rest.
        assert_eq!(thumb_geometry(400., 400., 0.), (200., 0.));
        // Scrolled to the end: the thumb's bottom meets the track's.
        let (thumb, top) = thumb_geometry(400., 400., 400.);
        assert_eq!(top + thumb, 400.);
        // A very long column still leaves a thumb a person can see.
        assert_eq!(thumb_geometry(400., 100_000., 0.).0, 28.);
    }
}
