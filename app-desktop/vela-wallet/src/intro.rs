//! The intro — three slides between the launch animation and the front door
//! (spec 020 on the web and the phones; spec 038 brings it to the desktop).
//!
//! It is shown ONCE, on the first run, and it is the thing a person meets
//! before they have any reason to trust us. So it argues rather than onboards:
//! no seed phrase, the keys stay yours, one address everywhere. The last slide
//! carries the two real ways in, because by then the argument is made.
//!
//! Same rail, same column, same buttons as Welcome and the create journey, so
//! the first screen a desktop visitor meets is composed like the second. The
//! state is a value ([`IntroState`]) and the screen a function of it
//! ([`render_intro`]); the page owns both and answers [`IntroEvent`]s through
//! a sink, exactly as the create flow does with `FlowSink`.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Div, FontWeight, ImageSource, InteractiveElement, MouseButton, MouseMoveEvent,
    ParentElement, SharedString, StatefulInteractiveElement as _, Styled, Window, div, img, px,
};

use crate::intro_art::{IntroArt, IntroArtCache, Palette, VIEW_H, VIEW_W};
use crate::loc::Loc;
use crate::theme::{
    self, FLOW_COLUMN_W, FLOW_GAP_LG, FLOW_GAP_MD, GAP_HERO_CTA, GAP_WELCOME_CTA, Theme,
};
use crate::ui::{ButtonVariant, welcome_cta};

/// The three slides, in the order the argument is made — one art and two
/// corpus keys each, the same sequence every shell reads (`slides.ts`).
const SLIDES: [(IntroArt, &str, &str); 3] = [
    (
        IntroArt::NoSeedPhrase,
        "onboarding.intro.noSeedTitle",
        "onboarding.intro.noSeedBody",
    ),
    (
        IntroArt::KeysAreYours,
        "onboarding.intro.custodyTitle",
        "onboarding.intro.custodyBody",
    ),
    (
        IntroArt::OneAddress,
        "onboarding.intro.chainsTitle",
        "onboarding.intro.chainsBody",
    ),
];

/// The illustration's drawn width; height follows the contract's viewBox.
const ART_W: f32 = 200.;
/// A drag shorter than this is a hesitation, not a page turn — a quarter of
/// the slide, the same threshold both ways so it never feels lopsided.
const TURN_FRACTION: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IntroEvent {
    Next,
    Prev,
    Skip,
    Create,
    SignIn,
    DragStart(f32),
    DragMove(f32),
    DragEnd,
}

/// How the intro was left — the page marks it seen and routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntroExit {
    Skip,
    Create,
    SignIn,
}

pub type IntroSink = Rc<dyn Fn(IntroEvent, &mut Window, &mut App)>;

#[derive(Default)]
pub struct IntroState {
    index: usize,
    /// Where the pointer went down, while a drag is live.
    drag_from: Option<f32>,
    /// Pixels the track is dragged from its resting position; 0 when settled.
    drag_px: f32,
    art: IntroArtCache,
}

impl IntroState {
    pub fn index(&self) -> usize {
        self.index
    }

    fn last(&self) -> bool {
        self.index + 1 == SLIDES.len()
    }

    fn go(&mut self, next: isize) {
        self.index = next.clamp(0, SLIDES.len() as isize - 1) as usize;
    }

    /// Apply one event. `Some` when the intro is over.
    pub fn apply(&mut self, event: IntroEvent) -> Option<IntroExit> {
        match event {
            IntroEvent::Next => self.go(self.index as isize + 1),
            IntroEvent::Prev => self.go(self.index as isize - 1),
            IntroEvent::Skip => return Some(IntroExit::Skip),
            IntroEvent::Create => return Some(IntroExit::Create),
            IntroEvent::SignIn => return Some(IntroExit::SignIn),
            IntroEvent::DragStart(x) => {
                self.drag_from = Some(x);
                self.drag_px = 0.;
            }
            IntroEvent::DragMove(x) => {
                if let Some(from) = self.drag_from {
                    let raw = x - from;
                    // Rubber-band at the two ends: there is nothing past them,
                    // and a track that slides freely into empty space says
                    // there is.
                    let overshoot = (self.index == 0 && raw > 0.) || (self.last() && raw < 0.);
                    self.drag_px =
                        if overshoot { raw / 3. } else { raw }.clamp(-FLOW_COLUMN_W, FLOW_COLUMN_W);
                }
            }
            IntroEvent::DragEnd => {
                if self.drag_from.take().is_some() {
                    if self.drag_px.abs() > FLOW_COLUMN_W * TURN_FRACTION {
                        let step = if self.drag_px < 0. { 1 } else { -1 };
                        self.go(self.index as isize + step);
                    }
                    self.drag_px = 0.;
                }
            }
        }
        None
    }
}

/// The intro's column — the rail is the page's, as on every onboarding screen.
pub fn render_intro(state: &mut IntroState, theme: &Theme, loc: &Loc, sink: IntroSink) -> Div {
    let last = state.last();
    let index = state.index;
    let total = SLIDES.len();
    let palette = Palette {
        line: theme.fg_subtle,
        accent: theme.accent,
        paper: theme.bg_base,
    };
    let art_h = ART_W * VIEW_H / VIEW_W;

    // -- header: the escape hatch, gone on the last slide whose buttons ARE
    //    the way out ---------------------------------------------------------
    let header = div()
        .w_full()
        .h(px(44.))
        .flex()
        .justify_end()
        .items_center()
        .when(!last, |el| {
            let sink = sink.clone();
            el.child(
                div()
                    .id("intro-skip")
                    .px(px(FLOW_GAP_MD))
                    .cursor_pointer()
                    .text_size(theme::text_body())
                    .text_color(theme.fg_muted)
                    .hover(|s| s.text_color(theme.fg_base))
                    .on_click(move |_, window, cx| sink(IntroEvent::Skip, window, cx))
                    .child(loc.t("onboarding.intro.skip")),
            )
        });

    // -- the track: three cells, one column wide each, moved as one ----------
    let offset = -(index as f32) * FLOW_COLUMN_W + state.drag_px;
    let mut track = div()
        .flex()
        .flex_row()
        .w(px(FLOW_COLUMN_W * total as f32))
        .ml(px(offset));
    for (art, title_key, body_key) in SLIDES {
        let image = state.art.image(art, palette, ART_W as u32);
        track = track.child(
            div()
                .w(px(FLOW_COLUMN_W))
                .flex_none()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(FLOW_GAP_LG))
                .px(px(FLOW_GAP_LG))
                .child(
                    img(ImageSource::Render(image))
                        .w(px(ART_W))
                        .h(px(art_h))
                        .flex_none(),
                )
                .child(
                    div()
                        .text_size(theme::text_hero_long())
                        .line_height(theme::line_height_hero_long())
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .text_center()
                        .child(loc.t(title_key)),
                )
                .child(
                    div()
                        .text_size(theme::text_flow_sub())
                        .line_height(theme::line_height_flow_sub())
                        .text_color(theme.fg_muted)
                        .text_center()
                        .child(loc.t(body_key)),
                ),
        );
    }
    let viewport = {
        let down = sink.clone();
        let moved = sink.clone();
        let up = sink.clone();
        let up_out = sink.clone();
        div()
            .id("intro-viewport")
            .w(px(FLOW_COLUMN_W))
            .overflow_hidden()
            .cursor_grab()
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                down(
                    IntroEvent::DragStart(f32::from(event.position.x)),
                    window,
                    cx,
                );
            })
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    moved(
                        IntroEvent::DragMove(f32::from(event.position.x)),
                        window,
                        cx,
                    );
                }
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                up(IntroEvent::DragEnd, window, cx)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                up_out(IntroEvent::DragEnd, window, cx)
            })
            .child(track)
    };

    // -- footer: the dots (a readout of something the person did), then the
    //    way on — or, on the last slide, the two ways in --------------------
    let mut dots = div().flex().flex_row().gap(px(8.)).justify_center();
    for i in 0..total {
        dots = dots.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if i == index {
            theme.fg_base
        } else {
            theme.fg_subtle
        }));
    }
    let page_of: SharedString = loc.t_vars(
        "onboarding.intro.pageOf",
        &[("current", (index + 1) as f64), ("total", total as f64)],
    );
    let _ = page_of; // the dots carry it visually; the label is the screen reader's, which gpui has no channel for yet

    let actions = if last {
        let create = sink.clone();
        let sign_in = sink.clone();
        div()
            .flex()
            .flex_row()
            .gap(px(GAP_WELCOME_CTA))
            .child(welcome_cta(
                "intro-create",
                ButtonVariant::Primary,
                loc.t("onboarding.welcome.createWallet"),
                true,
                theme,
                move |_, window, cx| create(IntroEvent::Create, window, cx),
            ))
            .child(welcome_cta(
                "intro-sign-in",
                ButtonVariant::Secondary,
                loc.t("onboarding.welcome.alreadyHaveWallet"),
                true,
                theme,
                move |_, window, cx| sign_in(IntroEvent::SignIn, window, cx),
            ))
    } else {
        let next = sink.clone();
        div().flex().flex_row().child(welcome_cta(
            "intro-next",
            ButtonVariant::Secondary,
            loc.t("onboarding.intro.next"),
            true,
            theme,
            move |_, window, cx| next(IntroEvent::Next, window, cx),
        ))
    };

    div()
        .w_full()
        .max_w(px(FLOW_COLUMN_W))
        .flex()
        .flex_col()
        .child(header)
        .child(viewport)
        .child(div().mt(px(FLOW_GAP_LG)).child(dots))
        .child(actions.mt(px(GAP_HERO_CTA)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_is_clamped_and_the_last_slide_knows_it() {
        let mut state = IntroState::default();
        assert_eq!(state.apply(IntroEvent::Prev), None);
        assert_eq!(state.index(), 0);
        state.apply(IntroEvent::Next);
        state.apply(IntroEvent::Next);
        assert!(state.last());
        state.apply(IntroEvent::Next);
        assert_eq!(state.index(), 2, "no fourth slide");
    }

    /// A quarter of the column turns the page; less is a hesitation; the ends
    /// rubber-band and never turn past themselves.
    #[test]
    fn a_drag_turns_the_page_only_past_a_quarter() {
        let mut state = IntroState::default();
        state.apply(IntroEvent::DragStart(300.));
        state.apply(IntroEvent::DragMove(300. - FLOW_COLUMN_W * 0.2));
        state.apply(IntroEvent::DragEnd);
        assert_eq!(state.index(), 0, "a fifth is a hesitation");

        state.apply(IntroEvent::DragStart(300.));
        state.apply(IntroEvent::DragMove(300. - FLOW_COLUMN_W * 0.3));
        state.apply(IntroEvent::DragEnd);
        assert_eq!(state.index(), 1);

        // Dragging right on the first slide rubber-bands: a third of the way.
        let mut first = IntroState::default();
        first.apply(IntroEvent::DragStart(0.));
        first.apply(IntroEvent::DragMove(90.));
        assert_eq!(first.drag_px, 30.);
        first.apply(IntroEvent::DragEnd);
        assert_eq!(first.index(), 0);
        assert_eq!(first.drag_px, 0., "settled");
    }

    #[test]
    fn the_three_exits_end_it() {
        let mut state = IntroState::default();
        assert_eq!(state.apply(IntroEvent::Skip), Some(IntroExit::Skip));
        assert_eq!(state.apply(IntroEvent::Create), Some(IntroExit::Create));
        assert_eq!(state.apply(IntroEvent::SignIn), Some(IntroExit::SignIn));
    }
}
