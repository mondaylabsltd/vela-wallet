//! The centred dialog (078 X-04): the web's `settings/ui/Dialog.svelte` and
//! the scrim every modal on it sits on.
//!
//! The desktop grew a dozen dialogs one at a time — padding 28, radius 20,
//! a scrim of the canvas at .55, no shadow — and each decided for itself
//! whether a click outside closed it. The web has one shape: 520 wide, padding
//! 24, radius 16, a hairline and `--shadow-lg`, on `color.fixed.backdrop`.
//! This is that shape, so a dialog is drawn once and the rest can move onto it.

use gpui::{
    App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, InteractiveElement as _, ParentElement,
    Pixels, SharedString, Stateful, StatefulInteractiveElement as _, Styled, Window, div, hsla, px,
};

use crate::theme::{self, Theme};

/// `--layout-settingsDialogW`: the centred dialog.
pub const DIALOG_W: f32 = theme::SETTINGS_DIALOG_W;
/// `--layout-promptCard`: a prompt's card — the identicon viewer, sign-out.
/// 440 holds a 42-character address in the mono face on one line.
pub const PROMPT_CARD_W: f32 = 440.;
/// `--space-3xl`, all round.
const PAD: f32 = 24.;
/// `--space-4xl`: the round close button.
const CLOSE: f32 = 32.;
/// `margin-bottom: var(--space-xl)` under the header.
const HEADER_GAP: f32 = 16.;

/// `--shadow-lg`: `0 4px 16px 0 rgba(26,26,24,.08)` — the web's, not gpui's
/// Tailwind one, which is twice as dark and reads as a floating window.
pub fn shadow_lg() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.), px(4.), hsla(60. / 360., 0.04, 0.1, 0.08)).blur_radius(px(16.))]
}

/// The layer under a dialog: `color.fixed.backdrop`, and nothing beneath it
/// takes a click or a wheel notch while it is up. A scrim that only dims is
/// not a scrim — the row under it still opens.
pub fn scrim(id: impl Into<ElementId>, theme: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.backdrop)
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
}

/// The card itself: raised, hairline, radius 16, padding 24, the shadow.
///
/// It occludes, so a click on the card is not also a click on the scrim's
/// dismiss behind it.
pub fn card(id: impl Into<ElementId>, theme: &Theme, width: f32) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .w(px(width))
        .flex()
        .flex_col()
        .p(px(PAD))
        .rounded(px(16.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border_card)
        .shadow(shadow_lg())
}

/// The web's `Dialog`: title 20 bold, a 32 round close on sunken, and a body
/// that scrolls inside `max-height: 80%` of the window while the header stays.
///
/// `close` is the ✕ and the scrim both, as on the web; Escape is the page's
/// key handler, which knows which layer is on top.
#[allow(clippy::too_many_arguments, reason = "one dialog, every part named")]
pub fn dialog(
    id: &'static str,
    theme: &Theme,
    window: &Window,
    title: SharedString,
    close_icon: impl gpui::IntoElement,
    body: impl gpui::IntoElement,
    scroll: &gpui::ScrollHandle,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let close = std::rc::Rc::new(close);
    let close_x = std::rc::Rc::clone(&close);
    let header = div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(12.))
        .mb(px(HEADER_GAP))
        .child(
            div()
                .min_w(px(0.))
                .text_size(theme::text_panel_title())
                .font_weight(FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(title),
        )
        .child(
            div()
                .id((id, 1u64))
                .size(px(CLOSE))
                .flex_none()
                .rounded_full()
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(move |event, window, cx| close_x(event, window, cx))
                .child(close_icon),
        );

    // `max-height: 80%` of the window, less the chrome the body does not own.
    let body_max: Pixels = px((f32::from(window.viewport_size().height) * 0.8
        - PAD * 2.
        - CLOSE
        - HEADER_GAP)
        .max(160.));
    let content = div()
        .relative()
        .w_full()
        .child(
            div()
                .id((id, 2u64))
                .track_scroll(scroll)
                .w_full()
                .max_h(body_max)
                .overflow_y_scroll()
                .child(body),
        )
        .children(crate::ui::vertical_scrollbar(theme, scroll));

    scrim(id, theme)
        .on_click(move |event, window, cx| close(event, window, cx))
        .child(
            card((id, 0u64), theme, DIALOG_W)
                .child(header)
                .child(content),
        )
}
