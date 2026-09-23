//! Account-name field (spec 014 Form pattern): optional label, single-line
//! editable well, over-length hint (A3), optional helper caption. An empty
//! label or helper renders NOTHING rather than an empty box with its own
//! margin — the create screen passes both empty since spec 019, because its
//! heading already names the field. Editing is the minimal gpui
//! idiom — a focus handle plus `on_key_down` appending `key_char`s and
//! handling backspace, with a styled-div caret. Composed text input (IME) is
//! a documented limitation of this pure-UI phase; the wiring feature owns a
//! real input if one lands upstream.

use crate::theme::{self, FLOW_CARET_W, FLOW_GAP_MD, FLOW_GAP_SM, INPUT_H, RADIUS_FIELD, Theme};
use gpui::{
    App, Div, ElementId, FocusHandle, FontWeight, InteractiveElement, KeyDownEvent, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

/// Already-resolved copy for the field (components know nothing about i18n).
pub struct NameFieldStrings {
    pub label: SharedString,
    pub placeholder: SharedString,
    pub helper: SharedString,
    pub too_long_hint: SharedString,
}

pub fn name_field(
    theme: &Theme,
    strings: &NameFieldStrings,
    value: &str,
    too_long: bool,
    focus: &FocusHandle,
    window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> Div {
    text_field(
        "name-field",
        theme,
        strings,
        value,
        too_long,
        false,
        focus,
        window,
        on_change,
    )
}

/// The same well, with its own id and an optional mask.
///
/// `id` because two fields can share a screen — the endpoint surface sits over
/// a flow that already has a name field, and a duplicate gpui element id makes
/// the second one unclickable. `mask` because a security key's PIN is a secret
/// that should not be shoulder-readable, and it is the only value this app
/// takes that is.
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn text_field(
    id: impl Into<ElementId>,
    theme: &Theme,
    strings: &NameFieldStrings,
    value: &str,
    too_long: bool,
    mask: bool,
    focus: &FocusHandle,
    window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> Div {
    let focused = focus.is_focused(window);
    let border = if too_long {
        theme.error_base
    } else if focused {
        theme.outline_strong
    } else {
        theme.divider
    };

    let shown = if mask {
        "•".repeat(value.chars().count())
    } else {
        value.to_owned()
    };
    let text: Div = if value.is_empty() {
        div()
            .text_size(theme::text_flow_sub())
            .text_color(theme.fg_subtle)
            .child(strings.placeholder.clone())
    } else {
        div()
            .text_size(theme::text_flow_sub())
            .text_color(theme.fg_base)
            .child(SharedString::from(shown))
    };

    // The row itself has to be clipped too, not just the text inside it: a
    // flex item's `min-width` is `auto`, so a 42-character address in a
    // 400px dialog grew the row past the well and painted across the card
    // behind it. Visible from the moment ⌘V could fill the field in one go.
    let mut inner = div()
        .flex()
        .items_center()
        .min_w(px(0.))
        .overflow_hidden()
        .child(
            div()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(text),
        );
    if focused {
        // Caret: a styled bar after the text (no blink — nothing timed here).
        inner = inner.child(
            div()
                .w(px(FLOW_CARET_W))
                .h(px(INPUT_H / 2.5))
                .flex_none()
                .bg(theme.accent),
        );
    }

    let well = {
        let current = value.to_owned();
        let focus_for_click = focus.clone();
        div()
            .id(id)
            .track_focus(focus)
            .h(px(INPUT_H))
            .w_full()
            .rounded(px(RADIUS_FIELD))
            // v2 fills the field with the PAGE colour, not the well: the
            // hairline is what makes it a field, and a second surface tone
            // under it only muddies the column.
            .bg(theme.bg_base)
            .border_1()
            .border_color(border)
            .px(px(FLOW_GAP_MD))
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .child(inner)
            .on_click(move |_, window, cx| focus_for_click.focus(window, cx))
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                // ⌘V. The one chord this minimal input owes a person: every
                // value these fields take — an address, a URL, an RPC
                // endpoint, a contract — arrives from somewhere else, and
                // until 2026-09-23 a paste-an-address dialog answered ⌘V with
                // nothing. A newline is dropped rather than typed: these are
                // single-line wells, and a pasted trailing newline is what
                // turns a good address into one the core refuses.
                if ks.modifiers.platform && !ks.modifiers.control && !ks.modifiers.alt {
                    if ks.key != "v" {
                        return;
                    }
                    let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                        return;
                    };
                    let pasted: String = pasted.chars().filter(|c| !c.is_control()).collect();
                    if pasted.is_empty() {
                        return;
                    }
                    on_change(format!("{current}{pasted}"), window, cx);
                    return;
                }
                if ks.modifiers.control || ks.modifiers.alt {
                    return;
                }
                let mut next = current.clone();
                if ks.key == "backspace" {
                    if next.pop().is_none() {
                        return;
                    }
                } else {
                    match &ks.key_char {
                        Some(ch) if !ch.chars().any(char::is_control) => next.push_str(ch),
                        _ => return,
                    }
                }
                on_change(next, window, cx);
            })
    };

    let mut col = div().flex().flex_col();
    if !strings.label.is_empty() {
        col = col.child(
            // v2's field label: tiny, uppercase and muted — a caption over the
            // field, not a heading beside it. Rendered only when there is one:
            // the name screen's own heading already says "name your wallet", so
            // it passes an empty label rather than restate it in smaller type.
            div()
                .text_size(theme::text_section_label())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(SharedString::from(strings.label.to_uppercase())),
        );
    }
    col = col.child(div().mt(px(FLOW_GAP_SM)).child(well));

    if too_long {
        // A3: the red line slots in WITHOUT displacing the field above it and
        // coexists with the helper caption below (spec edge case).
        col = col.child(
            div()
                .mt(px(FLOW_GAP_SM))
                .text_size(theme::text_flow_caption())
                .text_color(theme.error_base)
                .child(strings.too_long_hint.clone()),
        );
    }
    if strings.helper.is_empty() {
        return col;
    }
    col.child(
        div()
            .mt(px(FLOW_GAP_SM))
            .text_size(theme::text_flow_caption())
            .line_height(theme::line_height_body())
            .text_color(theme.fg_muted)
            .child(strings.helper.clone()),
    )
}
