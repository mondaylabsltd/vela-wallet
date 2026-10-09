//! Outcome status badge — the one circle behind every result state's glyph
//! (spec 014, 6 variants). The glyph is the text `!` except the timeout
//! clock, which is drawn with `PathBuilder` — no SVG assets exist in this
//! shell (research D7), and gpui would render them monochrome anyway.

use crate::outcome::BadgeVariant;
use crate::theme::{self, BADGE_CIRCLE, RING_STROKE, Theme};
use gpui::{
    Bounds, Div, FontWeight, Hsla, ParentElement, PathBuilder, Pixels, Point, Styled, Window,
    canvas, div, px,
};

pub fn status_badge(theme: &Theme, variant: BadgeVariant) -> Div {
    let (bg, fg) = match variant {
        BadgeVariant::Warning | BadgeVariant::Timeout => (theme.warning_soft, theme.warning_base),
        BadgeVariant::Error => (theme.error_soft, theme.error_base),
        BadgeVariant::Info => (theme.info_soft, theme.info_base),
    };

    let circle = div()
        .size(px(BADGE_CIRCLE))
        .flex_none()
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center();

    match glyph(variant) {
        None => circle.child(
            div().size(px(BADGE_CIRCLE / 2.)).child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint_clock(bounds, fg, window);
                    },
                )
                .size_full(),
            ),
        ),
        Some(glyph) => circle.child(
            div()
                .text_size(theme::text_badge_glyph())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(fg)
                .child(glyph),
        ),
    }
}

/// The text a badge draws, or `None` for the timeout, whose clock is painted.
///
/// Error, Warning and Info all draw the exclamation: the difference the
/// person reads is the colour, and a second glyph would be a second thing to
/// learn for the same fact. Error used to draw `×` — and a red × in a disc
/// at the top of a sheet reads as the button that closes it, which this one
/// is not (issue 460).
pub(crate) fn glyph(variant: BadgeVariant) -> Option<&'static str> {
    match variant {
        BadgeVariant::Timeout => None,
        BadgeVariant::Error | BadgeVariant::Warning | BadgeVariant::Info => Some("!"),
    }
}

/// Clock face: a stroked circle plus hour/minute hands at ten-past-ten-ish,
/// matching the E3 mock's simple outline glyph.
fn paint_clock(b: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let size = f32::from(b.size.width).min(f32::from(b.size.height));
    let stroke = RING_STROKE / 2.;
    let cx = f32::from(b.origin.x) + f32::from(b.size.width) / 2.;
    let cy = f32::from(b.origin.y) + f32::from(b.size.height) / 2.;
    let r = size / 2. - stroke;
    let at = |x: f32, y: f32| Point::new(px(cx + x), px(cy + y));

    // Face: two half arcs make the full circle.
    let mut pb = PathBuilder::stroke(px(stroke));
    pb.move_to(at(0., -r));
    pb.arc_to(Point::new(px(r), px(r)), px(0.), false, true, at(0., r));
    pb.arc_to(Point::new(px(r), px(r)), px(0.), false, true, at(0., -r));
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }

    // Hands: minute up, hour out to the right.
    let mut pb = PathBuilder::stroke(px(stroke));
    pb.move_to(at(0., -r * 0.55));
    pb.line_to(at(0., 0.));
    pb.line_to(at(r * 0.42, r * 0.12));
    if let Ok(path) = pb.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue 460: an error says "!", never the close glyph — the sheet's own
    /// ✕ and the window's are the only crosses a person should try to press.
    #[test]
    fn an_error_badge_is_an_exclamation_not_a_close() {
        assert_eq!(glyph(BadgeVariant::Error), Some("!"));
        for close in ["×", "✕", "x", "X"] {
            assert_ne!(glyph(BadgeVariant::Error), Some(close));
        }
        assert_eq!(glyph(BadgeVariant::Warning), glyph(BadgeVariant::Error));
        assert_eq!(glyph(BadgeVariant::Info), glyph(BadgeVariant::Error));
        assert_eq!(glyph(BadgeVariant::Timeout), None, "the clock is painted");
    }
}
