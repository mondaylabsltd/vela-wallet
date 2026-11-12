//! Hold to confirm (R22, review 1-3) — the ONE held confirm this shell draws:
//! the dApp signing sheet's confirm, the trusted-page hand-off's Open, and
//! every multisig sign / propose / approve / execute press whose door says
//! `hold_ms > 0`.
//!
//! When it is held, and for how long, is the core's (`sign_confirm::posture`
//! — every "… anyway" and every danger, `HOLD_TO_CONFIRM_MS`); this file only
//! draws the gesture:
//!
//! - **pointer**: press and hold; a fill grows over `hold_ms`; letting go
//!   early does nothing; the hold completing fires the confirm, once.
//! - **keyboard and assistive technology** (WCAG 2.5.1 / 2.5.7 — a held
//!   gesture is never the only way): an Enter / Space activation, or a screen
//!   reader's or switch's click (AccessKit's `Click` action), ARMS the confirm
//!   and says `componentsUi.signing.confirmAgain` ("Press again to confirm")
//!   — as its accessible name and on the line under it; a second activation
//!   within `CONFIRM_AGAIN_WINDOW_MS` confirms; past the window it disarms.
//!
//! A hold or an arm belongs to the request it began on (its `subject`): a
//! confirm that is no longer drawn held — shut meanwhile, gone, or now
//! another request's — never fires from it. A plain press (`hold_ms == 0`)
//! is not drawn here at all: the caller draws its button exactly as before.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AccessibleAction, App, ClickEvent, Div, ElementId, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px, relative,
};

use crate::theme::{self, Theme};

/// How often the fill is redrawn while a hold is under way.
const FRAME: Duration = Duration::from_millis(16);

/// A held confirm last drawn longer ago than this is not on screen any more
/// (shut, gone): a hold under way on it does not complete.
const DRAWN_WITHIN: Duration = Duration::from_millis(250);

/// What a held confirm does when it fires.
pub type Fire = Rc<dyn Fn(&mut Window, &mut App)>;

/// §39 (A1): told when a held confirm begins (`true`: a hold's start, or the
/// assistive path's first press — armed) and when it is let go without
/// firing (`false`: released early, disarmed after its window, or no longer
/// drawn). The signing sheet holds the fee it drew still in between.
pub type Engage = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The two lines a held confirm says (resolved by the caller, the corpus's
/// `componentsUi.signing.holdToConfirm` and `…confirmAgain`).
#[derive(Clone, Debug)]
pub struct HoldWords {
    pub hold: SharedString,
    pub again: SharedString,
}

/// Where a hold stands, when the fill asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    /// Still held, not long enough yet.
    Holding,
    /// Held for the whole `hold_ms`, on the confirm still drawn for the
    /// subject it began on: the confirm fires, once.
    Complete,
    /// Let go, another press began, or the confirm is no longer the one
    /// pressed: nothing happens.
    Released,
}

/// An accessible activation's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// The first: armed, "Press again to confirm".
    Armed,
    /// The second, within the window, on the same subject: the confirm fires.
    Confirm,
}

/// One held confirm's state. Pure — the clock and what is drawn are handed
/// in — so the rules are tested without a window.
#[derive(Clone, Debug, Default)]
pub struct HoldState {
    /// The press under way: its token, its subject and when it began.
    pressed: Option<(u64, SharedString, Instant)>,
    /// When the accessible path armed it, and for which subject.
    armed: Option<(SharedString, Instant)>,
    next: u64,
}

impl HoldState {
    /// A pointer went down on the confirm drawn for `subject`: a hold
    /// begins. Its token names this hold — a later press is another one.
    pub fn press(&mut self, now: Instant, subject: &SharedString) -> u64 {
        self.next += 1;
        self.pressed = Some((self.next, subject.clone(), now));
        self.next
    }

    /// The pointer let go (inside or outside): an unfinished hold does
    /// nothing.
    pub fn release(&mut self) {
        self.pressed = None;
    }

    /// The fill's question: is hold `token` still held, and for long enough?
    /// `drawn` is the subject the confirm is drawn held for now (`None`: not
    /// drawn held — shut, or gone). `Complete` is answered once, and only on
    /// the subject the hold began on — the hold ends there either way.
    pub fn poll(
        &mut self,
        token: u64,
        now: Instant,
        hold_ms: u32,
        drawn: Option<&SharedString>,
    ) -> Poll {
        let Some((held, subject, began)) = self.pressed.as_ref() else {
            return Poll::Released;
        };
        if *held != token {
            return Poll::Released;
        }
        if drawn != Some(subject) {
            self.pressed = None;
            return Poll::Released;
        }
        if now.saturating_duration_since(*began) >= ms(hold_ms) {
            self.pressed = None;
            return Poll::Complete;
        }
        Poll::Holding
    }

    /// How far the fill has grown, 0 to 1, on the confirm drawn for
    /// `subject`.
    #[must_use]
    pub fn progress(&self, now: Instant, hold_ms: u32, subject: &SharedString) -> f32 {
        let Some((_, pressed, began)) = self.pressed.as_ref() else {
            return 0.0;
        };
        if pressed != subject {
            return 0.0;
        }
        if hold_ms == 0 {
            return 1.0;
        }
        let done = now.saturating_duration_since(*began).as_secs_f32() / ms(hold_ms).as_secs_f32();
        done.clamp(0.0, 1.0)
    }

    /// A keyboard or assistive activation on the confirm drawn for
    /// `subject`: the first arms, a second within `window_ms` on the same
    /// subject confirms (and disarms).
    pub fn activate(&mut self, now: Instant, window_ms: u32, subject: &SharedString) -> Activation {
        if self.armed(now, window_ms, subject) {
            self.armed = None;
            Activation::Confirm
        } else {
            self.armed = Some((subject.clone(), now));
            Activation::Armed
        }
    }

    /// Armed for `subject`, and still within its window.
    #[must_use]
    pub fn armed(&self, now: Instant, window_ms: u32, subject: &SharedString) -> bool {
        self.armed.as_ref().is_some_and(|(armed, at)| {
            armed == subject && now.saturating_duration_since(*at) < ms(window_ms)
        })
    }

    /// Neither a hold under way nor an arm standing: the confirm is let go.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.pressed.is_none() && self.armed.is_none()
    }

    /// Past the window, the arm is dropped; `true` when it was.
    pub fn disarm_if_due(&mut self, now: Instant, window_ms: u32) -> bool {
        let due = self
            .armed
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= ms(window_ms));
        if due {
            self.armed = None;
        }
        due
    }
}

fn ms(ms: u32) -> Duration {
    Duration::from_millis(u64::from(ms))
}

/// Every held confirm on screen, by its id — one process-wide registry, so
/// any page that draws one holds its state across frames — and, for each,
/// which subject it was last drawn held for, and when.
#[derive(Default)]
pub struct Holds {
    states: HashMap<SharedString, HoldState>,
    drawn: RefCell<HashMap<SharedString, (SharedString, Instant)>>,
}

impl gpui::Global for Holds {}

fn with_state<R>(cx: &mut App, id: &SharedString, f: impl FnOnce(&mut HoldState) -> R) -> R {
    f(cx.default_global::<Holds>()
        .states
        .entry(id.clone())
        .or_default())
}

/// The clock a hold and an arm are measured on: gpui's own
/// (`BackgroundExecutor::now`) — the wall clock in the app, and the test
/// scheduler's under `TestAppContext`, so the 5 s disarm is tested by moving
/// the clock rather than by waiting.
fn now(cx: &App) -> Instant {
    cx.background_executor().now()
}

/// The subject `id` is drawn held for right now, if it is.
fn drawn_now(cx: &App, id: &SharedString, now: Instant) -> Option<SharedString> {
    let holds = cx.try_global::<Holds>()?;
    let drawn = holds.drawn.borrow();
    let (subject, at) = drawn.get(id)?;
    (now.saturating_duration_since(*at) < DRAWN_WITHIN).then(|| subject.clone())
}

/// Whether the confirm `id` stands armed for `subject` now ("Press again to
/// confirm").
#[cfg(test)]
#[must_use]
pub fn armed_now(cx: &App, id: &str, subject: &str) -> bool {
    let window_ms = vela_core::app::sign_confirm::CONFIRM_AGAIN_WINDOW_MS;
    let at = now(cx);
    cx.try_global::<Holds>()
        .and_then(|holds| holds.states.get(id))
        .is_some_and(|state| state.armed(at, window_ms, &SharedString::from(subject.to_owned())))
}

/// This frame draws `id` held for `subject`: (fill, armed).
fn drawing(
    cx: &App,
    id: &SharedString,
    subject: &SharedString,
    hold_ms: u32,
    window_ms: u32,
) -> (f32, bool) {
    let now = now(cx);
    let Some(holds) = cx.try_global::<Holds>() else {
        return (0.0, false);
    };
    holds
        .drawn
        .borrow_mut()
        .insert(id.clone(), (subject.clone(), now));
    holds.states.get(id).map_or((0.0, false), |state| {
        (
            state.progress(now, hold_ms, subject),
            state.armed(now, window_ms, subject),
        )
    })
}

/// `id` was let go (a hold over without firing, an arm run out): the
/// caller hears it — only once nothing else holds it (a new press, an arm).
fn let_go(id: &SharedString, engage: Option<&Engage>, window: &mut Window, cx: &mut App) {
    if let Some(engage) = engage
        && with_state(cx, id, |state| state.idle())
    {
        engage(false, window, cx);
    }
}

/// A pointer went down: the hold begins, and its fill is redrawn until it
/// completes (the confirm fires) or is let go.
fn begin_hold(
    id: SharedString,
    subject: SharedString,
    hold_ms: u32,
    fire: Fire,
    engage: Option<Engage>,
    window: &mut Window,
    cx: &mut App,
) {
    let at = now(cx);
    let token = with_state(cx, &id, |state| state.press(at, &subject));
    cx.default_global::<Holds>()
        .drawn
        .borrow_mut()
        .insert(id.clone(), (subject, at));
    if let Some(engage) = engage.as_ref() {
        engage(true, window, cx);
    }
    window.refresh();
    window
        .spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(FRAME).await;
                let done = cx
                    .update(|window, cx| {
                        let at = now(cx);
                        let drawn = drawn_now(cx, &id, at);
                        let poll = with_state(cx, &id, |state| {
                            state.poll(token, at, hold_ms, drawn.as_ref())
                        });
                        window.refresh();
                        match poll {
                            Poll::Holding => false,
                            Poll::Complete => {
                                fire(window, cx);
                                true
                            }
                            Poll::Released => {
                                let_go(&id, engage.as_ref(), window, cx);
                                true
                            }
                        }
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
}

/// A keyboard or assistive activation (module doc).
fn activate(
    id: SharedString,
    subject: &SharedString,
    window_ms: u32,
    fire: &Fire,
    engage: Option<Engage>,
    window: &mut Window,
    cx: &mut App,
) {
    let at = now(cx);
    match with_state(cx, &id, |state| state.activate(at, window_ms, subject)) {
        Activation::Confirm => fire(window, cx),
        Activation::Armed => {
            if let Some(engage) = engage.as_ref() {
                engage(true, window, cx);
            }
            window.refresh();
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(ms(window_ms)).await;
                    cx.update(|window, cx| {
                        let at = now(cx);
                        if with_state(cx, &id, |state| state.disarm_if_due(at, window_ms)) {
                            let_go(&id, engage.as_ref(), window, cx);
                            window.refresh();
                        }
                    })
                    .ok();
                })
                .detach();
        }
    }
}

/// The held confirm: `face` (the button as it looks enabled — its fill, its
/// words) under a fill that grows while it is held, and the line under it
/// ("Hold to confirm", or "Press again to confirm" once armed). `label` is
/// its accessible name; `fill` the colour the hold grows in; `subject` what
/// it confirms (the request, the transaction) — a hold or an arm never
/// carries over to another. Answers the pointer, the keyboard and assistive
/// technology (module doc). `engage` (§39, A1) hears the confirm begin and
/// be let go without firing.
///
/// A tab stop: Tab and ⇧Tab reach it through the shell's focus walk
/// ([`crate::ui::tab_walk`], bound in `app_menu`), and Enter or Space on it
/// arms it — R22 follow-up r22-5, where nothing in the shell bound Tab and
/// the keyboard path was unreachable.
#[allow(
    clippy::too_many_arguments,
    reason = "the control, what it confirms, its words and its act"
)]
pub fn held_confirm(
    id: impl Into<SharedString>,
    subject: impl Into<SharedString>,
    theme: &Theme,
    face: impl IntoElement,
    fill: Hsla,
    label: SharedString,
    hold_ms: u32,
    words: &HoldWords,
    fire: Fire,
    engage: Option<Engage>,
    cx: &App,
) -> Div {
    let id: SharedString = id.into();
    let subject: SharedString = subject.into();
    let window_ms = vela_core::app::sign_confirm::CONFIRM_AGAIN_WINDOW_MS;
    let (progress, armed) = drawing(cx, &id, &subject, hold_ms, window_ms);
    let name = if armed {
        words.again.clone()
    } else {
        label.clone()
    };
    let (press_id, key_fire, a11y_fire) = (id.clone(), fire.clone(), fire.clone());
    let (key_id, a11y_id) = (id.clone(), id.clone());
    let (up_id, out_id) = (id.clone(), id.clone());
    let (press_subject, key_subject, a11y_subject) =
        (subject.clone(), subject.clone(), subject.clone());
    let (press_engage, key_engage, a11y_engage) = (engage.clone(), engage.clone(), engage);
    let control = div()
        .id(ElementId::Name(id))
        .role(Role::Button)
        .aria_label(name)
        .aria_description(words.hold.clone())
        .focusable()
        .tab_stop(true)
        .relative()
        .overflow_hidden()
        .rounded(px(theme::RADIUS_CTA))
        .cursor_pointer()
        .child(face)
        .child(
            // The hold, growing over the face from its leading edge.
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(relative(progress))
                .bg(fill),
        )
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            begin_hold(
                press_id.clone(),
                press_subject.clone(),
                hold_ms,
                fire.clone(),
                press_engage.clone(),
                window,
                cx,
            );
        })
        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
            with_state(cx, &up_id, HoldState::release);
            window.refresh();
        })
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
            with_state(cx, &out_id, HoldState::release);
            window.refresh();
        })
        // Enter / Space on the focused control (a pointer's click is the
        // hold above, never a confirm of its own).
        .on_click(move |event: &ClickEvent, window, cx| {
            if event.is_keyboard() {
                activate(
                    key_id.clone(),
                    &key_subject,
                    window_ms,
                    &key_fire,
                    key_engage.clone(),
                    window,
                    cx,
                );
            }
        })
        // A screen reader's or a switch's activation — handled here, so it
        // is never turned into an instant pointer press and release.
        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            activate(
                a11y_id.clone(),
                &a11y_subject,
                window_ms,
                &a11y_fire,
                a11y_engage.clone(),
                window,
                cx,
            );
        });
    div().flex().flex_col().gap(px(6.)).child(control).child(
        div()
            .w_full()
            .text_center()
            .text_size(theme::text_label())
            .text_color(theme.fg_muted)
            .child(if armed {
                words.again.clone()
            } else {
                words.hold.clone()
            }),
    )
}

/// The fill a hold grows in over `danger`'s face or the value face: white,
/// translucent — the face's own colour, lighter, in both themes.
#[must_use]
pub fn hold_fill() -> Hsla {
    Hsla::from(gpui::rgb(0xffffff)).opacity(0.28)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::sign_confirm::{CONFIRM_AGAIN_WINDOW_MS, HOLD_TO_CONFIRM_MS};

    const HOLD: u32 = HOLD_TO_CONFIRM_MS;
    const WINDOW: u32 = CONFIRM_AGAIN_WINDOW_MS;

    fn after(start: Instant, ms_: u64) -> Instant {
        start + Duration::from_millis(ms_)
    }

    fn rid(id: &str) -> SharedString {
        SharedString::from(id.to_owned())
    }

    /// Held for the whole `hold_ms`: the confirm fires, once, and the fill
    /// grew over the hold.
    #[test]
    fn a_hold_that_completes_confirms_once() {
        let (start, a) = (Instant::now(), rid("rid-1"));
        let mut state = HoldState::default();
        let token = state.press(start, &a);
        assert_eq!(
            state.poll(token, after(start, 700), HOLD, Some(&a)),
            Poll::Holding
        );
        let half = state.progress(after(start, 750), HOLD, &a);
        assert!((half - 0.5).abs() < 0.01, "{half}");
        assert_eq!(
            state.poll(token, after(start, u64::from(HOLD)), HOLD, Some(&a)),
            Poll::Complete
        );
        // Fired once: the hold is over.
        assert_eq!(
            state.poll(token, after(start, u64::from(HOLD) + 16), HOLD, Some(&a)),
            Poll::Released
        );
        assert!(state.progress(after(start, 2_000), HOLD, &a).abs() < f32::EPSILON);
    }

    /// Let go early: nothing — and a new press is a new hold, never the
    /// old one completing.
    #[test]
    fn an_early_release_does_nothing() {
        let (start, a) = (Instant::now(), rid("rid-1"));
        let mut state = HoldState::default();
        let first = state.press(start, &a);
        state.release();
        assert_eq!(
            state.poll(first, after(start, u64::from(HOLD) + 100), HOLD, Some(&a)),
            Poll::Released
        );
        let second = state.press(after(start, 200), &a);
        assert_eq!(
            state.poll(first, after(start, u64::from(HOLD) + 300), HOLD, Some(&a)),
            Poll::Released,
            "the first press does not complete through the second"
        );
        assert_eq!(
            state.poll(second, after(start, u64::from(HOLD) + 199), HOLD, Some(&a)),
            Poll::Holding
        );
    }

    /// A hold on a confirm that is shut meanwhile, or now another request's,
    /// never completes.
    #[test]
    fn a_hold_never_completes_on_a_confirm_no_longer_drawn_for_it() {
        let (start, a, b) = (Instant::now(), rid("rid-1"), rid("rid-2"));
        let done = after(start, u64::from(HOLD) + 1);
        let mut state = HoldState::default();
        let token = state.press(start, &a);
        assert_eq!(state.poll(token, done, HOLD, None), Poll::Released, "shut");
        let token = state.press(start, &a);
        assert_eq!(
            state.poll(token, done, HOLD, Some(&b)),
            Poll::Released,
            "another request's"
        );
        assert!(state.progress(done, HOLD, &b).abs() < f32::EPSILON);
    }

    /// The accessible path: the first activation arms (and says "Press
    /// again to confirm"), a second within the window confirms — on the
    /// same subject only.
    #[test]
    fn the_assistive_path_arms_then_confirms() {
        let (start, a, b) = (Instant::now(), rid("rid-1"), rid("rid-2"));
        let mut state = HoldState::default();
        assert_eq!(state.activate(start, WINDOW, &a), Activation::Armed);
        assert!(state.armed(after(start, 10), WINDOW, &a));
        assert!(!state.armed(after(start, 10), WINDOW, &b));
        assert_eq!(
            state.activate(after(start, 4_000), WINDOW, &a),
            Activation::Confirm
        );
        assert!(!state.armed(after(start, 4_001), WINDOW, &a));
        // Armed for one request, pressed again on another: armed anew.
        assert_eq!(state.activate(start, WINDOW, &a), Activation::Armed);
        assert_eq!(
            state.activate(after(start, 100), WINDOW, &b),
            Activation::Armed
        );
    }

    /// Past the window the arm is gone: the next activation arms again,
    /// never confirms.
    #[test]
    fn the_arm_expires_after_its_window() {
        let (start, a) = (Instant::now(), rid("rid-1"));
        let mut state = HoldState::default();
        assert_eq!(state.activate(start, WINDOW, &a), Activation::Armed);
        let late = after(start, u64::from(WINDOW) + 1);
        assert!(!state.armed(late, WINDOW, &a));
        assert!(state.disarm_if_due(late, WINDOW));
        assert!(!state.disarm_if_due(late, WINDOW), "dropped once");
        assert_eq!(state.activate(late, WINDOW, &a), Activation::Armed);
    }
}

/// R22 follow-up r22-5 (S3): the held confirm is reachable from the keyboard
/// in a real gpui window — the shell's own bindings (`app_menu`) and focus
/// walk ([`crate::ui::tab_walk`]) over the one held confirm this shell draws.
/// Tab reaches it; Enter arms it ("Press again to confirm"); the arm runs out
/// after `CONFIRM_AGAIN_WINDOW_MS` on gpui's clock; Enter, Enter confirms —
/// once. And the confirm's `engage` hears each begin and each let-go.
#[cfg(test)]
mod keyboard {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, KeyUpEvent,
        Keystroke, ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext,
        Window, div,
    };
    use vela_core::app::sign_confirm::{CONFIRM_AGAIN_WINDOW_MS, HOLD_TO_CONFIRM_MS};

    const ID: &str = "signing-confirm";
    const SUBJECT: &str = "rid-1";

    /// A sheet as the shell draws one: a root that holds the window's focus
    /// (as `WalletPage` does) and walks it, over a held confirm.
    struct Sheet {
        focus: FocusHandle,
        fired: Rc<Cell<u32>>,
        engaged: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for Sheet {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = crate::theme::Theme::light();
            let fired = self.fired.clone();
            let engaged = self.engaged.clone();
            let words = super::HoldWords {
                hold: "Hold to confirm".into(),
                again: "Press again to confirm".into(),
            };
            crate::ui::tab_walk(
                div()
                    .id("sheet")
                    .track_focus(&self.focus)
                    .size_full()
                    .child(super::held_confirm(
                        ID,
                        SUBJECT,
                        &theme,
                        div()
                            .h(gpui::px(48.))
                            .w(gpui::px(240.))
                            .child("Sign anyway"),
                        super::hold_fill(),
                        "Sign anyway".into(),
                        HOLD_TO_CONFIRM_MS,
                        &words,
                        Rc::new(move |_, _| fired.set(fired.get() + 1)),
                        Some(Rc::new(move |on, _, _| engaged.borrow_mut().push(on))),
                        cx,
                    )),
            )
        }
    }

    /// A key pressed and let go on whatever has the keyboard — a key down
    /// then a key up, as the platform sends them (gpui turns an Enter or a
    /// Space pair into the focused control's keyboard click).
    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap_or_else(|e| unreachable!("{key}: {e}"));
        cx.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke });
    }

    fn armed(cx: &mut VisualTestContext) -> bool {
        cx.update(|_, cx| super::armed_now(cx, ID, SUBJECT))
    }

    #[gpui::test]
    fn tab_reaches_the_held_confirm_and_enter_arms_disarms_and_confirms(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.bind_keys(crate::app_menu::wallet_key_bindings(cfg!(
                target_os = "macos"
            )));
            cx.bind_keys(crate::app_menu::focus_key_bindings());
        });
        let fired = Rc::new(Cell::new(0));
        let engaged = Rc::new(RefCell::new(Vec::new()));
        let (sheet, cx) = cx.add_window_view({
            let (fired, engaged) = (fired.clone(), engaged.clone());
            move |window, cx| {
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                Sheet {
                    focus,
                    fired,
                    engaged,
                }
            }
        });
        let root = cx.update(|_, cx| sheet.read(cx).focus.clone());
        assert!(
            cx.update(|window, _| root.is_focused(window)),
            "the root has the keyboard"
        );

        // Tab: the keyboard leaves the root for the held confirm.
        cx.simulate_keystrokes("tab");
        assert!(
            !cx.update(|window, _| root.is_focused(window)),
            "Tab reached no tab stop: the held confirm is unreachable from the keyboard"
        );

        // Enter: armed — "Press again to confirm" — and the sheet hears it
        // begin; nothing fired.
        press(cx, "enter");
        assert!(armed(cx), "Enter on the focused confirm arms it");
        assert_eq!(fired.get(), 0);
        assert_eq!(*engaged.borrow(), [true]);

        // Past the window: disarmed, and the sheet hears it let go.
        cx.executor().advance_clock(Duration::from_millis(
            u64::from(CONFIRM_AGAIN_WINDOW_MS) + 1,
        ));
        cx.run_until_parked();
        assert!(!armed(cx), "disarmed after its window");
        assert_eq!(*engaged.borrow(), [true, false]);
        assert_eq!(fired.get(), 0);

        // Enter, Enter within the window: confirmed, once.
        press(cx, "enter");
        assert!(armed(cx));
        press(cx, "enter");
        assert_eq!(fired.get(), 1, "the second press confirms");
        assert!(!armed(cx), "and the arm is spent");
        assert_eq!(*engaged.borrow(), [true, false, true]);

        // ⇧Tab walks back the other way (one stop here: the same control).
        cx.simulate_keystrokes("shift-tab");
        assert!(!cx.update(|window, _| root.is_focused(window)));
    }
}
