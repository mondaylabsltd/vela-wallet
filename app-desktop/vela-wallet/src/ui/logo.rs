//! The Vela mark, drawn with `PathBuilder` from the exact geometry of
//! `docs/design/onboarding/logo-*.svg` (viewBox 258×260).
//!
//! Not `svg()`: the default AssetSource resolves nothing (silently), and gpui
//! renders SVGs as monochrome masks anyway — the mark needs three fills
//! (research.md D2). The two sails are theme-invariant; the hull themes.

use crate::theme::{self, Theme, WORDMARK_TRACKING};
use gpui::{
    Bounds, Div, FontWeight, Hsla, ParentElement, PathBuilder, Pixels, Point, Styled, Window,
    canvas, div, px,
};

/// Logo viewBox, the coordinate space of the path constants below. This is
/// the artwork's TIGHT CROP.
const VB_W: f32 = 258.;
const VB_H: f32 = 260.;

/// The square box the design draws that artwork inside.
///
/// v2's brand row is `<svg viewBox="0 0 420 420" style="width: 60px">` whose
/// paths span 258×260 of those 420 — so at a 60px box the mark is ~37px of
/// artwork with ~11px of air on each side, and the 12px gap to the wordmark is
/// measured from the BOX. Scaling the tight crop to 60 instead renders the
/// mark 1.6× oversized, which is what made it tower over a 19px wordmark.
const VB_BOX: f32 = 420.;

pub fn vela_mark(theme: &Theme, size: Pixels) -> Div {
    let (sail_a, sail_b, hull) = (theme.logo_sail_a, theme.logo_sail_b, theme.logo_hull);
    div().size(size).flex_none().child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                paint_mark(bounds, sail_a, sail_b, hull, window);
            },
        )
        .size_full(),
    )
}

/// The wordmark beside the mark: `VELA WALLET`, uppercase, `letter-spacing:
/// .11em` (docs/design/onboarding-new).
///
/// One element per glyph, because **gpui has no letter-spacing** — the string
/// does not appear anywhere in its text system, so there is no property to set
/// and no shortcut to take. A gap between per-glyph elements is what tracking
/// would have been, and at .11em on an all-caps latin wordmark the kerning
/// pairs it gives up are worth nothing next to the tracking itself.
///
/// Never localized: this is the product's name, and every locale renders it.
pub fn vela_wordmark(theme: &Theme) -> Div {
    let size = theme::text_wordmark();
    let tracking = px(f32::from(size) * WORDMARK_TRACKING);
    // A typical sans word space is ~0.26em; the tracking sits on top of it, as
    // it does between letters.
    let word_gap = px(f32::from(size) * 0.26 + f32::from(tracking));

    let word = |text: &'static str| {
        let mut row = div().flex().items_center().gap(tracking);
        for glyph in text.chars() {
            row = row.child(div().child(glyph.to_string()));
        }
        row
    };

    div()
        .flex()
        .items_center()
        .gap(word_gap)
        .text_size(size)
        // BOLD, as the web's `.wordmark` (`--weight-bold`) — spec 038 finding 8.
        .font_weight(FontWeight::BOLD)
        .text_color(theme.fg_base)
        .child(word("VELA"))
        .child(word("WALLET"))
}

fn paint_mark(b: Bounds<Pixels>, sail_a: Hsla, sail_b: Hsla, hull: Hsla, window: &mut Window) {
    // Uniform scale, centered. The divisor is the design's BOX, not the tight
    // crop: `size` is the box the mark sits in, and the artwork is inset in it.
    let scale = f32::from(b.size.width).min(f32::from(b.size.height)) / VB_BOX;
    let ox = f32::from(b.origin.x) + (f32::from(b.size.width) - VB_W * scale) / 2.;
    let oy = f32::from(b.origin.y) + (f32::from(b.size.height) - VB_H * scale) / 2.;
    let at = move |x: f32, y: f32| Point::new(px(ox + x * scale), px(oy + y * scale));

    // Big sail: M122,0 C70,53 38,118 18,187 L122,187 Z
    let mut pb = PathBuilder::fill();
    pb.move_to(at(122., 0.));
    pb.cubic_bezier_to(at(18., 187.), at(70., 53.), at(38., 118.));
    pb.line_to(at(122., 187.));
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, sail_a);
    }

    // Small sail: M142,42 C193,75 225,128 240,187 L142,187 Z
    let mut pb = PathBuilder::fill();
    pb.move_to(at(142., 42.));
    pb.cubic_bezier_to(at(240., 187.), at(193., 75.), at(225., 128.));
    pb.line_to(at(142., 187.));
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, sail_b);
    }

    // Hull: M0,207 L258,207 C243,240 211,260 165,260 L92,260 C49,260 16,240 0,207 Z
    let mut pb = PathBuilder::fill();
    pb.move_to(at(0., 207.));
    pb.line_to(at(258., 207.));
    pb.cubic_bezier_to(at(165., 260.), at(243., 240.), at(211., 260.));
    pb.line_to(at(92., 260.));
    pb.cubic_bezier_to(at(0., 207.), at(49., 260.), at(16., 240.));
    pb.close();
    if let Ok(path) = pb.build() {
        window.paint_path(path, hull);
    }
}
