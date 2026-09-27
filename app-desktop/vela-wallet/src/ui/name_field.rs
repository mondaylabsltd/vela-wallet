//! The text wells: the account-name field (spec 014 Form pattern: optional
//! label, single-line well, over-length hint (A3), optional helper caption —
//! an empty label or helper renders NOTHING rather than an empty box with its
//! own margin), and the send form's figure, its recipient line, the search
//! input and the multi-line well, which share its keys.
//!
//! Every one of them edits through [`super::editor`]: a caret that goes where
//! the arrows, Home/End and the mouse send it, a real selection, and text —
//! the IME's composition included — arriving through the platform's input
//! handler. They used to be caret-less (typing appended, the arrows were
//! swallowed, a click only focused, an IME had nowhere to compose); the
//! signatures stayed, so no call site changed.

use std::rc::Rc;

use super::editor::{self, EditorStyle, OnChange, Wrap};
use crate::theme::{self, FLOW_GAP_MD, FLOW_GAP_SM, INPUT_H, RADIUS_FIELD, Theme};
use gpui::{
    App, Div, ElementId, FocusHandle, FontWeight, InteractiveElement, Keystroke, MouseButton,
    ParentElement, SharedString, Styled, Window, div, px,
};

/// The four clipboard chords a text well owes a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditChord {
    SelectAll,
    Copy,
    Cut,
    Paste,
}

/// Which clipboard chord `ks` is, if any — ⌘ on macOS, Ctrl on Windows and
/// Linux (`Modifiers::secondary`).
///
/// Every field here first read `modifiers.platform`, which is ⌘ on a Mac and
/// the WINDOWS key on Windows: Ctrl+V fell through to "a chord with Ctrl is
/// not text" and did nothing, and Ctrl+A/C/X were never read on any desktop.
/// Shift is allowed (Ctrl+Shift+V pastes in most apps); Alt, or ⌘ and Ctrl
/// together, is some other chord.
#[must_use]
pub fn edit_chord(ks: &Keystroke) -> Option<EditChord> {
    let m = &ks.modifiers;
    if !m.secondary() || m.alt || (m.platform && m.control) {
        return None;
    }
    match ks.key.as_str() {
        "a" => Some(EditChord::SelectAll),
        "c" => Some(EditChord::Copy),
        "x" => Some(EditChord::Cut),
        "v" => Some(EditChord::Paste),
        _ => None,
    }
}

/// The wells' shared look for the text itself: the UI face, the accent caret
/// and the accent-tinted selection the old all-or-nothing selection wore.
fn style(theme: &Theme, size: gpui::Pixels, line_height: gpui::Pixels) -> EditorStyle {
    EditorStyle {
        family: theme::font_ui().into(),
        placeholder_family: theme::font_ui().into(),
        weight: FontWeight::NORMAL,
        size,
        line_height,
        color: theme.fg_base,
        placeholder: theme.fg_subtle,
        caret: theme.accent,
        selection: theme.accent.opacity(0.28),
    }
}

/// A well's two handlers — the mouse places the caret, the keys move and
/// edit — on whatever element is the well.
fn wire<E: InteractiveElement>(
    well: E,
    focus: &FocusHandle,
    wrap: Wrap,
    mask: bool,
    on_change: OnChange,
) -> E {
    well.on_mouse_down(
        MouseButton::Left,
        editor::mouse_down(focus, Rc::clone(&on_change)),
    )
    .on_key_down(editor::key_down(focus, wrap, mask, on_change))
}

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

    editor::sync(focus, value);
    let on_change: OnChange = Rc::new(on_change);
    let size = theme::text_flow_sub();
    let text = editor::editor(
        focus,
        strings.placeholder.clone(),
        style(theme, size, size * 1.4),
        Wrap::None,
        Rc::clone(&on_change),
    )
    .masked(mask);

    let well = wire(
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
            // The row itself is clipped too: a flex item's `min-width` is
            // `auto`, and a 42-character address in a 400px dialog grew the
            // row past the well. The editor scrolls to its caret instead.
            .child(div().flex_1().min_w(px(0.)).child(text)),
        focus,
        Wrap::None,
        mask,
        on_change,
    );

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

/// The send form's amount, as the web's `AmountInput` draws it: the figure
/// large and centred, the unit after it in a lighter face — and the same keys
/// as every well. The figure is as wide as it is, so the pair centres
/// together; past the room there is, it scrolls to its caret.
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn hero_amount_field(
    id: impl Into<ElementId>,
    theme: &Theme,
    value: &str,
    placeholder: SharedString,
    unit: SharedString,
    focus: &FocusHandle,
    _window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    let rung = theme::amount_hero_rung(value.chars().count(), unit.chars().count());
    let size = theme::text_amount_hero(rung);
    editor::sync(focus, value);
    let on_change: OnChange = Rc::new(on_change);
    let figure = editor::editor(
        focus,
        placeholder,
        EditorStyle {
            weight: FontWeight::BOLD,
            ..style(theme, size, theme::line_amount_hero())
        },
        Wrap::None,
        Rc::clone(&on_change),
    )
    .fit_content();
    // As wide as the column and no wider: in the form's centred column a row
    // is otherwise sized to its content, so a long figure spilled past BOTH
    // edges — the unit with it.
    wire(
        div()
            .id(id)
            .track_focus(focus)
            .cursor_text()
            .w_full()
            .h(theme::line_amount_hero())
            .flex()
            .items_center()
            .justify_center()
            .gap(px(2.))
            .min_w(px(0.))
            .overflow_hidden()
            .whitespace_nowrap()
            .child(figure)
            .child(
                div()
                    .flex_none()
                    .ml(px(8.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(theme::text_amount_unit(rung))
                    .text_color(theme.fg_muted)
                    .child(unit),
            ),
        focus,
        Wrap::None,
        false,
        on_change,
    )
}

/// A well with no well: the text and its caret, for a field that sits inside
/// a card of its own (the send form's recipient). An address runs to two
/// even lines rather than being clipped — a cut address hides exactly the
/// characters a poisoning attack changes.
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn bare_text_field(
    id: impl Into<ElementId>,
    theme: &Theme,
    value: &str,
    placeholder: SharedString,
    focus: &FocusHandle,
    _window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    editor::sync(focus, value);
    let on_change: OnChange = Rc::new(on_change);
    let size = theme::text_row_sub();
    let text = editor::editor(
        focus,
        placeholder,
        EditorStyle {
            family: theme::font_mono().into(),
            ..style(theme, size, size * 1.45)
        },
        Wrap::Halves,
        Rc::clone(&on_change),
    );
    wire(
        div()
            .id(id)
            .track_focus(focus)
            .cursor_text()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(36.))
            .flex()
            .items_center()
            .overflow_hidden()
            .child(div().flex_1().min_w(px(0.)).child(text)),
        focus,
        Wrap::Halves,
        false,
        on_change,
    )
}

/// The typing half of a search field (078 X-05) — the web's `<input
/// type="search">`: one line in the UI face, at 13, the placeholder in
/// `fg-subtle` until something is typed. The well around it (height, fill,
/// glyph, focus edge) is the caller's, because the flows and the contacts
/// header draw different wells around the same input.
pub fn search_input(
    id: impl Into<ElementId>,
    theme: &Theme,
    value: &str,
    placeholder: SharedString,
    focus: &FocusHandle,
    _window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    editor::sync(focus, value);
    let on_change: OnChange = Rc::new(on_change);
    let size = theme::text_row_sub();
    let text = editor::editor(
        focus,
        placeholder,
        style(theme, size, size * 1.45),
        Wrap::None,
        Rc::clone(&on_change),
    );
    wire(
        div()
            .id(id)
            .track_focus(focus)
            .cursor_text()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(div().flex_1().min_w(px(0.)).child(text)),
        focus,
        Wrap::None,
        false,
        on_change,
    )
}

/// A multi-line well — the web's `<textarea>` (078 S-03): `rows` lines tall
/// at rest and growing with what is typed, the words wrapping, Enter a new
/// line, ↑/↓ between the lines drawn (wrapped ones too), a paste keeping its
/// line breaks.
///
/// `inert` is the web's `inert`: while a report goes the well takes no
/// focus, draws no caret and hears no key or click — what is being sent does
/// not change under the spinner.
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn text_area(
    id: impl Into<ElementId>,
    theme: &Theme,
    value: &str,
    placeholder: SharedString,
    rows: u16,
    inert: bool,
    focus: &FocusHandle,
    window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    let focused = !inert && focus.is_focused(window);
    let size = theme::text_body();
    let line = size * 1.5;
    let wrap = Wrap::Soft { min_rows: rows };
    editor::sync(focus, value);
    let on_change: OnChange = Rc::new(on_change);
    let text = editor::editor(
        focus,
        placeholder,
        style(theme, size, line),
        wrap,
        Rc::clone(&on_change),
    );
    let well = div()
        .id(id)
        .w_full()
        .min_h(line * f32::from(rows) + px(24.))
        .p(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_sunken)
        .border_1()
        .border_color(if focused {
            theme.outline_strong
        } else {
            theme.divider
        })
        .child(text);
    if inert {
        return well;
    }
    wire(
        well.track_focus(focus).cursor_text(),
        focus,
        wrap,
        false,
        on_change,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(keys: &str) -> Option<EditChord> {
        edit_chord(&Keystroke::parse(keys).expect(keys))
    }

    /// ⌘ on a Mac, Ctrl everywhere else — and never the other one. Windows
    /// shipped reading ⌘ (its Windows key) for paste, so Ctrl+V did nothing.
    #[test]
    fn the_clipboard_chords_are_this_desktops_own() {
        let (own, other) = if cfg!(target_os = "macos") {
            ("cmd", "ctrl")
        } else {
            ("ctrl", "cmd")
        };
        for (key, expected) in [
            ("a", EditChord::SelectAll),
            ("c", EditChord::Copy),
            ("x", EditChord::Cut),
            ("v", EditChord::Paste),
        ] {
            assert_eq!(
                chord(&format!("{own}-{key}")),
                Some(expected),
                "{own}-{key}"
            );
            assert_eq!(chord(&format!("{other}-{key}")), None, "{other}-{key}");
        }
        assert_eq!(chord(&format!("{own}-shift-v")), Some(EditChord::Paste));
        assert_eq!(chord(&format!("{own}-alt-v")), None);
        assert_eq!(chord(&format!("{own}-{other}-v")), None);
        assert_eq!(chord(&format!("{own}-z")), None);
        assert_eq!(chord("v"), None);
    }
}
