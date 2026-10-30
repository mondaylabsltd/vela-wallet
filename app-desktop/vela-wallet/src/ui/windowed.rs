//! A long column drawn a window at a time, inside a [`super::SmoothScroll`]
//! column (native parity Phase 4).
//!
//! gpui lays the whole window out again on every frame of a scroll, so a
//! History of 200 transfers cost 12 ms a frame to lay out, measure and paint
//! — 80 fps where the display wants 120, on a list where 15 rows are ever on
//! screen. gpui's own `list` / `uniform_list` are scroll containers of their
//! own, with gpui's raw wheel handling; this keeps the column that already
//! scrolls the way a browser does (`smooth_scroll.rs`: the eased glide, the
//! resampled trackpad, the scrollbar, the resets to the top) and only changes
//! what is IN it:
//!
//! ```text
//!   ┌ spacer: Σ heights above the window ┐
//!   │ item first … item last-1           │  drawn: in view, ± OVERDRAW
//!   └ spacer: Σ heights below the window ┘
//! ```
//!
//! Heights are each item's own, measured the frame it is drawn
//! (`on_children_prepainted`) and kept by its key; an item never drawn yet
//! counts as the mean of those that were. A width change (a resize, the text
//! size) forgets them all. Items of a few kinds (a day's heading, a row) are
//! fine: the mean is only a guess for what nobody has looked at yet, and the
//! scrollbar settles as the column is scrolled through.
//!
//! What it costs: an item outside the window is not in the tree, so it is not
//! in the accessibility tree either — a screen reader reads the rows on
//! screen and the scroll moves it on, as with any virtualised list.
//!
//! The column must stack its items bare: no `gap`, no margins between them —
//! the spacers are the sums of the items' own boxes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{AnyElement, Div, ParentElement as _, ScrollHandle, SharedString, Styled as _, div, px};

/// Drawn beyond each edge of what is in view, px: two frames of the fastest
/// flick at 120 Hz with a screen's worth to spare, so a frame whose window was
/// worked out from the last frame's layout still has rows under it.
const OVERDRAW: f32 = 480.;
/// Below this many items everything is drawn: there is nothing to save, and
/// a short list stays exactly the column it always was.
const MIN_ITEMS: usize = 48;
/// The guess for an item before any has been measured: one activity row at
/// the default text size (64, `wallet::components::activity_row`).
const FIRST_GUESS: f32 = 64.;
/// Drawn on a column's first frame, before its viewport is known: enough to
/// fill the tallest window at the first guess.
const FIRST_FRAME_PX: f32 = 2400.;

/// One column's measurements. Clone it into the builder each frame; the
/// clones share one state, as [`super::SmoothScroll`]'s do.
#[derive(Clone, Default)]
pub struct Windowed(Rc<RefCell<State>>);

#[derive(Default)]
struct State {
    /// Each drawn item's height, by its key.
    heights: HashMap<SharedString, f32>,
    /// The width those were measured at.
    width: f32,
    /// Where the column starts inside the scrolled content, px from its top —
    /// the content above it (a heading, a filter) is not part of the window.
    top: Option<f32>,
}

impl Windowed {
    pub fn new() -> Self {
        Self::default()
    }

    /// The column: `keys[i]` names item `i` (stable across frames — an id,
    /// not an index, so a transfer arriving on top keeps every row's height),
    /// `item(i)` draws it. Only the items near what `scroll` shows are drawn.
    pub fn column(
        &self,
        scroll: &ScrollHandle,
        keys: &[SharedString],
        mut item: impl FnMut(usize) -> AnyElement,
    ) -> Div {
        let col = div().flex().flex_col();
        if keys.len() < MIN_ITEMS {
            return col.children((0..keys.len()).map(item));
        }
        let (heights, top) = {
            let state = self.0.borrow();
            (state.estimated(keys), state.top)
        };
        let viewport = f32::from(scroll.bounds().size.height);
        // px down the content the column's own top is seen from.
        let down = -f32::from(scroll.offset().y);
        let (lo, hi) = match top {
            Some(top) if viewport > 0. => (down - top - OVERDRAW, down - top + viewport + OVERDRAW),
            // Not laid out yet: the top of the list, and plenty of it.
            _ => (0., FIRST_FRAME_PX),
        };
        let span = window(&heights, lo, hi);
        let drawn: Vec<SharedString> = keys[span.first..span.last].to_vec();
        let state = self.0.clone();
        let handle = scroll.clone();
        col.on_children_prepainted(move |bounds, _, _| {
            state.borrow_mut().learn(&bounds, &drawn, &handle);
        })
        .child(div().flex_none().h(px(span.above)))
        .children((span.first..span.last).map(&mut item))
        .child(div().flex_none().h(px(span.below)))
    }
}

impl State {
    /// Every item's height: measured, else the mean of the measured ones.
    fn estimated(&self, keys: &[SharedString]) -> Vec<f32> {
        let guess = if self.heights.is_empty() {
            FIRST_GUESS
        } else {
            #[allow(clippy::cast_precision_loss, reason = "a count of rows")]
            let count = self.heights.len() as f32;
            self.heights.values().sum::<f32>() / count
        };
        keys.iter()
            .map(|key| self.heights.get(key).copied().unwrap_or(guess))
            .collect()
    }

    /// What this frame's layout said: the drawn items' heights, and where
    /// the column sits in the content. `bounds` are the column's children —
    /// the top spacer, the drawn items, the bottom spacer.
    fn learn(
        &mut self,
        bounds: &[gpui::Bounds<gpui::Pixels>],
        drawn: &[SharedString],
        scroll: &ScrollHandle,
    ) {
        let Some(spacer) = bounds.first() else {
            return;
        };
        let width = f32::from(spacer.size.width);
        if (width - self.width).abs() > 0.5 {
            self.heights.clear();
            self.width = width;
        }
        for (key, item) in drawn.iter().zip(bounds.iter().skip(1)) {
            self.heights
                .insert(key.clone(), f32::from(item.size.height));
        }
        // On screen = the viewport's top + the scroll (negative) + the place
        // in the content; the last is what stays put while the column moves.
        let viewport_top = f32::from(scroll.bounds().origin.y);
        let offset = f32::from(scroll.offset().y);
        self.top = Some(f32::from(spacer.origin.y) - viewport_top - offset);
    }
}

/// Which items a window `[lo, hi)` (px down the column) touches, and what is
/// left above and below them.
#[derive(Debug, PartialEq)]
struct Span {
    first: usize,
    last: usize,
    above: f32,
    below: f32,
}

fn window(heights: &[f32], lo: f32, hi: f32) -> Span {
    let mut y = 0.;
    let mut first = heights.len();
    let mut above = 0.;
    for (i, h) in heights.iter().enumerate() {
        if y + h > lo {
            first = i;
            above = y;
            break;
        }
        y += h;
    }
    if first == heights.len() {
        above = y;
    }
    let mut last = first;
    let mut y = above;
    while last < heights.len() && y < hi {
        y += heights[last];
        last += 1;
    }
    let below = heights[last..].iter().sum();
    Span {
        first,
        last,
        above,
        below,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_draws_what_it_touches_and_sums_the_rest() {
        let heights = [10., 10., 10., 10., 10.];
        assert_eq!(
            window(&heights, 15., 31.),
            Span {
                first: 1,
                last: 4,
                above: 10.,
                below: 10.
            }
        );
        // The top: nothing above.
        assert_eq!(window(&heights, -100., 5.).first, 0);
        assert_eq!(window(&heights, -100., 5.).above, 0.);
        // Past the end: nothing drawn, everything above.
        let past = window(&heights, 100., 200.);
        assert_eq!(
            (past.first, past.last, past.above, past.below),
            (5, 5, 50., 0.)
        );
    }

    #[test]
    fn spacers_and_drawn_items_always_add_up_to_the_column() {
        let heights: Vec<f32> = (0..300).map(|i| 40. + (i % 7) as f32 * 6.).collect();
        let total: f32 = heights.iter().sum();
        for lo in [-500., 0., 333., 4000., 12_000., 99_999.] {
            let span = window(&heights, lo, lo + 900.);
            let drawn: f32 = heights[span.first..span.last].iter().sum();
            assert!((span.above + drawn + span.below - total).abs() < 0.01);
        }
    }

    #[test]
    fn an_unmeasured_item_is_guessed_at_the_mean_of_the_measured() {
        let mut state = State::default();
        let keys: Vec<SharedString> = ["a", "b", "c"].map(SharedString::from).to_vec();
        assert_eq!(state.estimated(&keys), vec![FIRST_GUESS; 3]);
        state.heights.insert("a".into(), 20.);
        state.heights.insert("b".into(), 40.);
        assert_eq!(state.estimated(&keys), vec![20., 40., 30.]);
    }
}
