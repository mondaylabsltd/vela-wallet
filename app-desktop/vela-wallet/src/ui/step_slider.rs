//! A slider over a few fixed stops — the track of the text-size control.
//!
//! It behaves as the web's `input[type=range]` and every platform slider do,
//! because that is what a hand that has used one expects:
//!
//! - **The whole track is the target.** A press anywhere on it takes the
//!   thumb to the stop nearest the pointer. The first version made each stop
//!   its own click target around a six-pixel dot, and a press between two of
//!   them, or a little above the line, did nothing. The track is also taller
//!   than it lays out (`HIT_H` against `LAYOUT_H`), so a press that lands just
//!   above or below the dots still counts, and the row keeps its height.
//! - **Press and drag.** With the button down the thumb steps from stop to
//!   stop under the pointer, and each stop is handed over as it is reached —
//!   the caller puts it in force, so the page is its own preview. The drag
//!   is followed across the whole window, not only over the track.
//! - **The thumb answers the pointer.** Over the track it grows and a soft
//!   ring comes up around it; pressed, both grow again; and the tick a press
//!   would land on is marked before the press, so a person can see where it
//!   will go.
//!
//! Every motion is a critically damped spring, stepped once per frame in
//! [`StepSlider::render`], and dropped for a system that asks for less
//! motion: the thumb is then simply where it belongs.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Bounds, BoxShadow, DispatchPhase, Div, InteractiveElement as _, MouseButton,
    MouseMoveEvent, ParentElement as _, Pixels, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, div, hsla, px, relative,
};

use crate::theme::Theme;

/// How tall the track lays out — what the row around it makes room for.
const LAYOUT_H: f32 = 28.;
/// How tall it is to the pointer: the platforms' minimum target, reaching
/// into the row's padding above and below.
const HIT_H: f32 = 44.;
/// The first and last stops sit this far in from the track's ends, so the
/// thumb and its ring are whole at either end.
const INSET: f32 = 12.;

/// The thumb's diameter at rest, over the track, and pressed.
const THUMB: [f32; 3] = [16., 20., 22.];
/// The ring's diameter and strength at the same three moments. At rest it
/// is not there.
const RING: [f32; 3] = [16., 34., 40.];
const RING_ALPHA: [f32; 3] = [0., 0.10, 0.16];
/// A stop's tick, and the tick a press would land on.
const TICK: f32 = 6.;
const TICK_AIMED: f32 = 8.;

/// The springs' stiffness: the thumb's glide to a stop and the grow/shrink.
/// Both settle in about a tenth of a second.
const GLIDE_OMEGA: f32 = 32.;
const LIFT_OMEGA: f32 = 30.;
/// A frame longer than this is a stall, not motion to catch up on.
const MAX_STEP_S: f32 = 0.05;

/// The thumb's lift: at rest, under the pointer, pressed.
const REST: f32 = 0.;
const HOVER: f32 = 1.;
const PRESSED: f32 = 2.;

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// One slider's pointer state and motion. The view that draws the slider
/// keeps one of these for as long as the slider is on screen and clones it
/// into the element each frame; the clones share one state.
#[derive(Clone, Default)]
pub struct StepSlider {
    state: Rc<RefCell<State>>,
}

#[derive(Default)]
struct State {
    /// The track as last painted; a pointer is measured against it.
    bounds: Option<Bounds<Pixels>>,
    /// The pointer is over the track.
    hovered: bool,
    /// The stop the pointer is nearest, while it is over the track.
    aimed: Option<usize>,
    /// Pressed on the track and not let go yet.
    dragging: bool,
    /// The stop last handed over, so a drag hands each stop over once.
    committed: Option<usize>,
    /// The thumb's place along the stops (0 = first, 1 = last); `None` until
    /// the first frame puts it straight on its stop.
    place: Option<Motion>,
    /// `REST`, `HOVER` or `PRESSED`, sprung between.
    lift: Motion,
    last_frame: Option<Instant>,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    value: f32,
    velocity: f32,
}

impl State {
    /// The stop nearest `x`, a window position, or `None` before the track
    /// has been painted once.
    fn stop_at(&self, x: Pixels, steps: usize) -> Option<usize> {
        let bounds = self.bounds?;
        let rail = (f32::from(bounds.size.width) - 2. * INSET).max(1.);
        let share = (f32::from(x - bounds.left()) - INSET) / rail;
        Some(stop_of(share, steps))
    }

    /// Advances both springs to this frame. Returns whether either still
    /// moves, which is whether another frame is wanted.
    fn step(&mut self, place_target: f32, reduce_motion: bool) -> bool {
        let lift_target = if self.dragging {
            PRESSED
        } else if self.hovered {
            HOVER
        } else {
            REST
        };
        let place = self.place.get_or_insert(Motion {
            value: place_target,
            velocity: 0.,
        });
        if reduce_motion {
            *place = Motion {
                value: place_target,
                velocity: 0.,
            };
            self.lift = Motion {
                value: lift_target,
                velocity: 0.,
            };
            self.last_frame = None;
            return false;
        }
        let now = Instant::now();
        // The first frame of a motion has no frame before it: call it one.
        let dt = self.last_frame.map_or(1. / 120., |last| {
            now.duration_since(last).as_secs_f32().min(MAX_STEP_S)
        });
        *place = spring(*place, place_target, GLIDE_OMEGA, dt);
        self.lift = spring(self.lift, lift_target, LIFT_OMEGA, dt);
        let place_done = settle(place, place_target, 0.002);
        let lift_done = settle(&mut self.lift, lift_target, 0.01);
        let moving = !(place_done && lift_done);
        self.last_frame = moving.then_some(now);
        moving
    }
}

/// How the track looks on one frame.
#[derive(Clone, Copy)]
struct Look {
    /// The thumb's place along the stops, 0 to 1.
    place: f32,
    /// `REST` to `PRESSED`.
    lift: f32,
    /// A stop to mark as the one a press would land on.
    aimed: Option<usize>,
}

impl StepSlider {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The track, live: `on_pick` is handed a stop each time the press or
    /// the drag reaches one that is not `index`. It is expected to put that
    /// stop in force; the next frame's `index` is then that stop.
    pub fn render(
        &self,
        id: &'static str,
        theme: &Theme,
        steps: usize,
        index: usize,
        reduce_motion: bool,
        on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        let pick: Pick = Rc::new(on_pick);
        let (look, moving) = {
            let mut state = self.state.borrow_mut();
            state.committed = Some(index);
            let moving = state.step(place_of(index, steps), reduce_motion);
            let look = Look {
                place: state.place.map_or(place_of(index, steps), |m| m.value),
                lift: state.lift.value,
                aimed: state
                    .aimed
                    .filter(|aimed| !state.dragging && *aimed != index),
            };
            (look, moving)
        };

        let measure = self.state.clone();
        let follow = self.state.clone();
        let follow_pick = pick.clone();
        let press = self.state.clone();
        let press_pick = pick;
        let aim = self.state.clone();
        let hover = self.state.clone();
        let release = self.state.clone();
        let release_out = self.state.clone();

        track(theme, steps, look)
            .id(id)
            .h(px(HIT_H))
            // Taller to the pointer than to the row: the extra reaches into
            // the padding above and below instead of pushing it apart.
            .my(px(-(HIT_H - LAYOUT_H) / 2.))
            .cursor_pointer()
            .child(
                canvas(
                    move |bounds, window, _| {
                        measure.borrow_mut().bounds = Some(bounds);
                        if moving {
                            window.request_animation_frame();
                        }
                    },
                    move |_, _, window, _| {
                        // The drag is followed wherever the pointer goes,
                        // not only while it is over the track.
                        let state = follow.clone();
                        let pick = follow_pick.clone();
                        window.on_mouse_event(
                            move |event: &MouseMoveEvent, phase, window, cx| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }
                                let reached = {
                                    let mut state = state.borrow_mut();
                                    if !state.dragging {
                                        return;
                                    }
                                    // Let go somewhere no release reached us.
                                    if event.pressed_button != Some(MouseButton::Left) {
                                        state.dragging = false;
                                        None
                                    } else {
                                        state.stop_at(event.position.x, steps).filter(|stop| {
                                            let new = state.committed != Some(*stop);
                                            if new {
                                                state.committed = Some(*stop);
                                            }
                                            new
                                        })
                                    }
                                };
                                if let Some(stop) = reached {
                                    pick(stop, window, cx);
                                }
                                window.refresh();
                            },
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let reached = {
                    let mut state = press.borrow_mut();
                    state.dragging = true;
                    state.aimed = None;
                    state.stop_at(event.position.x, steps).filter(|stop| {
                        let new = state.committed != Some(*stop);
                        if new {
                            state.committed = Some(*stop);
                        }
                        new
                    })
                };
                if let Some(stop) = reached {
                    press_pick(stop, window, cx);
                }
                window.refresh();
                cx.stop_propagation();
            })
            .on_mouse_move(move |event, window, _| {
                let changed = {
                    let mut state = aim.borrow_mut();
                    let aimed = if state.dragging {
                        None
                    } else {
                        state.stop_at(event.position.x, steps)
                    };
                    let changed = aimed != state.aimed || !state.hovered;
                    state.aimed = aimed;
                    state.hovered = true;
                    changed
                };
                // Only when what is drawn changes: a move that stays nearest
                // the same stop redraws nothing.
                if changed {
                    window.refresh();
                }
            })
            .on_hover(move |hovered, window, _| {
                let mut state = hover.borrow_mut();
                state.hovered = *hovered;
                if !*hovered {
                    state.aimed = None;
                }
                window.refresh();
            })
            .on_mouse_up(MouseButton::Left, move |_, window, _| {
                release.borrow_mut().dragging = false;
                window.refresh();
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, _| {
                let mut state = release_out.borrow_mut();
                if state.dragging {
                    state.dragging = false;
                    window.refresh();
                }
            })
    }
}

/// The track drawn only, the thumb at rest on `index` — for the gallery and
/// the design surfaces, which have nothing to move it with.
#[must_use]
pub fn step_slider_picture(theme: &Theme, steps: usize, index: usize) -> Div {
    track(
        theme,
        steps,
        Look {
            place: place_of(index, steps),
            lift: REST,
            aimed: None,
        },
    )
    .h(px(LAYOUT_H))
}

/// The ticks, the ring and the thumb, in a box as wide as it is given. The
/// caller sets the height; everything is centred on it.
fn track(theme: &Theme, steps: usize, look: Look) -> Div {
    let thumb = lerp3(THUMB, look.lift);
    let ring = lerp3(RING, look.lift);
    let ring_alpha = lerp3(RING_ALPHA, look.lift);
    let mut rail = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(INSET))
        .right(px(INSET));
    for stop in 0..steps {
        let aimed = look.aimed == Some(stop);
        let (size, color) = if aimed {
            (TICK_AIMED, theme.fg_subtle)
        } else {
            (TICK, theme.outline_strong)
        };
        rail = rail.child(at(place_of(stop, steps), dot(size, color)));
    }
    // The web's thumb carries a shadow (`--shadow-md`); a lighter one here,
    // enough to lift it off the ring.
    let shadow = BoxShadow::new(px(0.), px(1.), hsla(0., 0., 0., 0.18)).blur_radius(px(3.));
    rail = rail
        .when(ring_alpha > 0.005, |rail| {
            rail.child(at(look.place, dot(ring, theme.fg_muted.opacity(ring_alpha))))
        })
        .child(at(
            look.place,
            dot(thumb, theme.fg_muted).shadow(vec![shadow]),
        ));
    div().relative().flex_1().min_w(px(0.)).child(rail)
}

/// A round dot.
fn dot(size: f32, color: gpui::Hsla) -> Div {
    div()
        .flex_none()
        .ml(px(-size / 2.))
        .size(px(size))
        .rounded_full()
        .bg(color)
}

/// `dot` centred at `place` (0 to 1) along the rail and on the track's
/// midline: a zero-wide column there, the dot pulled back half its width.
fn at(place: f32, dot: Div) -> Div {
    div()
        .absolute()
        .left(relative(place))
        .top_0()
        .bottom_0()
        .w(px(0.))
        .flex()
        .items_center()
        .child(dot)
}

/// Where stop `index` of `steps` sits along the rail, 0 to 1.
fn place_of(index: usize, steps: usize) -> f32 {
    if steps < 2 {
        return 0.;
    }
    index.min(steps - 1) as f32 / (steps - 1) as f32
}

/// The stop nearest `share` of the way along the rail — before the first
/// stop is the first, past the last the last.
fn stop_of(share: f32, steps: usize) -> usize {
    if steps < 2 {
        return 0;
    }
    let last = (steps - 1) as f32;
    (share.clamp(0., 1.) * last).round() as usize
}

/// Between the three values of a `REST`/`HOVER`/`PRESSED` table.
fn lerp3(table: [f32; 3], lift: f32) -> f32 {
    let lift = lift.clamp(REST, PRESSED);
    if lift <= HOVER {
        table[0] + (table[1] - table[0]) * lift
    } else {
        table[1] + (table[2] - table[1]) * (lift - HOVER)
    }
}

/// One frame of a critically damped spring pulling `m` to `target`, solved
/// exactly rather than integrated so a long frame cannot overshoot.
fn spring(m: Motion, target: f32, omega: f32, dt: f32) -> Motion {
    let gap = m.value - target;
    let lead = m.velocity + omega * gap;
    let decay = (-omega * dt).exp();
    Motion {
        value: target + (gap + lead * dt) * decay,
        velocity: (m.velocity - omega * lead * dt) * decay,
    }
}

/// Close enough and slow enough: put it exactly there and say it is still.
fn settle(m: &mut Motion, target: f32, within: f32) -> bool {
    if (m.value - target).abs() < within && m.velocity.abs() < within * 10. {
        *m = Motion {
            value: target,
            velocity: 0.,
        };
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_goes_to_the_nearest_stop_and_off_the_ends_to_the_end_stops() {
        assert_eq!(stop_of(0., 7), 0);
        assert_eq!(stop_of(0.49, 7), 3);
        // Halfway between stop 3 (0.5) and stop 4 (0.667).
        assert_eq!(stop_of(0.58, 7), 3);
        assert_eq!(stop_of(0.59, 7), 4);
        assert_eq!(stop_of(-0.3, 7), 0);
        assert_eq!(stop_of(1.4, 7), 6);
        assert_eq!(stop_of(0.7, 1), 0);
    }

    #[test]
    fn stops_are_even_from_end_to_end() {
        assert_eq!(place_of(0, 7), 0.);
        assert_eq!(place_of(3, 7), 0.5);
        assert_eq!(place_of(6, 7), 1.);
        assert_eq!(place_of(9, 7), 1.);
        for index in 0..7 {
            assert_eq!(stop_of(place_of(index, 7), 7), index);
        }
    }

    #[test]
    fn the_thumb_grows_over_the_track_and_more_when_pressed() {
        assert_eq!(lerp3(THUMB, REST), 16.);
        assert_eq!(lerp3(THUMB, HOVER), 20.);
        assert_eq!(lerp3(THUMB, PRESSED), 22.);
        assert_eq!(lerp3(THUMB, 0.5), 18.);
        assert_eq!(lerp3(RING_ALPHA, REST), 0.);
    }

    #[test]
    fn the_glide_arrives_without_overshooting() {
        let mut m = Motion {
            value: 0.,
            velocity: 0.,
        };
        let mut frames = 0;
        while !settle(&mut m, 1., 0.002) {
            m = spring(m, 1., GLIDE_OMEGA, 1. / 120.);
            assert!(m.value <= 1. + 1e-4, "overshot to {}", m.value);
            frames += 1;
            assert!(frames < 120, "never settled");
        }
        // A tenth of a second, give or take, at 120 Hz.
        assert!((8..40).contains(&frames), "{frames} frames");
    }

    #[test]
    fn reduced_motion_puts_the_thumb_straight_on_its_stop() {
        let mut state = State {
            place: Some(Motion {
                value: 0.,
                velocity: 0.,
            }),
            hovered: true,
            ..State::default()
        };
        assert!(!state.step(1., true));
        assert_eq!(state.place.unwrap().value, 1.);
        assert_eq!(state.lift.value, HOVER);
    }
}
