//! Account-name field (spec 014 Form pattern): optional label, single-line
//! editable well, over-length hint (A3), optional helper caption. An empty
//! label or helper renders NOTHING rather than an empty box with its own
//! margin — the create screen passes both empty since spec 019, because its
//! heading already names the field. Editing is the minimal gpui
//! idiom — a focus handle plus `on_key_down` appending `key_char`s and
//! handling backspace, with a styled-div caret — plus the four clipboard
//! chords ([`edit_chord`]) over an all-or-nothing selection. Composed text input (IME) is
//! a documented limitation of this pure-UI phase; the wiring feature owns a
//! real input if one lands upstream.

use std::cell::RefCell;

use crate::theme::{self, FLOW_CARET_W, FLOW_GAP_MD, FLOW_GAP_SM, INPUT_H, RADIUS_FIELD, Theme};
use gpui::{
    App, ClipboardItem, Div, ElementId, FocusHandle, FontWeight, InteractiveElement, KeyDownEvent,
    Keystroke, ParentElement, SharedString, StatefulInteractiveElement, Styled, WeakFocusHandle,
    Window, div, prelude::FluentBuilder as _, px,
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

thread_local! {
    /// The well whose whole value is selected, if any.
    ///
    /// These wells have no caret position — typing appends — so a selection
    /// is all or nothing, and one at a time, like focus. Kept here rather than
    /// in each form: thirteen call sites build a well, and a selection none of
    /// them asked for should not become a field on all of them. Keyed by the
    /// well's focus handle, and only honoured while that well has focus.
    static SELECTED: RefCell<Option<WeakFocusHandle>> = const { RefCell::new(None) };
}

fn is_selected(focus: &FocusHandle) -> bool {
    SELECTED.with_borrow(|selected| selected.as_ref().is_some_and(|weak| weak == focus))
}

fn select(focus: Option<&FocusHandle>) {
    SELECTED.with_borrow_mut(|selected| *selected = focus.map(FocusHandle::downgrade));
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
    let selected = focused && !value.is_empty() && is_selected(focus);
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
            .when(selected, |text| {
                text.bg(theme.accent.opacity(0.28)).rounded(px(2.))
            })
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
    if focused && !selected {
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
        let focus_for_keys = focus.clone();
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
            .on_click(move |_, window, cx| {
                // A click puts the caret back at the end, as it would anywhere.
                select(None);
                focus_for_click.focus(window, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                let selected = !current.is_empty() && is_selected(&focus_for_keys);
                if let Some(chord) = edit_chord(ks) {
                    match chord {
                        EditChord::SelectAll => {
                            if !current.is_empty() {
                                select(Some(&focus_for_keys));
                                window.refresh();
                            }
                        }
                        // A masked well holds a PIN: it may be pasted into,
                        // never read back out.
                        EditChord::Copy => {
                            if selected && !mask {
                                cx.write_to_clipboard(ClipboardItem::new_string(current.clone()));
                            }
                        }
                        EditChord::Cut => {
                            if selected && !mask {
                                cx.write_to_clipboard(ClipboardItem::new_string(current.clone()));
                                select(None);
                                on_change(String::new(), window, cx);
                            }
                        }
                        // Every value these wells take — an address, a URL, an
                        // RPC endpoint, a contract — arrives from somewhere
                        // else. A newline is dropped rather than typed: these
                        // are single-line wells, and a pasted trailing newline
                        // is what turns a good address into one the core
                        // refuses.
                        EditChord::Paste => {
                            let Some(pasted) =
                                cx.read_from_clipboard().and_then(|item| item.text())
                            else {
                                return;
                            };
                            let pasted: String =
                                pasted.chars().filter(|c| !c.is_control()).collect();
                            if pasted.is_empty() {
                                return;
                            }
                            let kept = if selected { "" } else { current.as_str() };
                            select(None);
                            on_change(format!("{kept}{pasted}"), window, cx);
                        }
                    }
                    return;
                }
                if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
                    return;
                }
                // A selection is replaced by whatever is typed over it.
                let mut next = if selected {
                    String::new()
                } else {
                    current.clone()
                };
                match ks.key.as_str() {
                    "backspace" | "delete" if selected => {}
                    "backspace" => {
                        if next.pop().is_none() {
                            return;
                        }
                    }
                    "left" | "right" | "home" | "end" | "escape" => {
                        if selected {
                            select(None);
                            window.refresh();
                        }
                        return;
                    }
                    _ => match &ks.key_char {
                        Some(ch) if !ch.chars().any(char::is_control) => next.push_str(ch),
                        _ => return,
                    },
                }
                select(None);
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
