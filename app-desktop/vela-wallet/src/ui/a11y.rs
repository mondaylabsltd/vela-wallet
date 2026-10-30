//! The accessibility tree (N-D1): what VoiceOver, Narrator and Orca read.
//!
//! gpui at this pin ships AccessKit, and the app runs with it ON
//! (`gpui_platform::application()`, never `new_inaccessible`) — but a node is
//! reported only when an element has an id AND a role, and bare text children
//! are not reported at all (`gpui::_accessibility`). So every shared control
//! names itself here, with the words a sighted person reads on it: the
//! corpus's own, in the person's language.
//!
//! One vocabulary for the whole app, so a button is a button everywhere:
//!
//! - [`Accessible::a11y_button`] — anything pressed (CTAs, pills, icon
//!   buttons, tiles);
//! - [`Accessible::a11y_tab`] — a section of the sidebar or of Settings,
//!   `selected` when it is the one shown;
//! - [`Accessible::a11y_item`] — a row in a list (an asset, a network, a
//!   contact) read as one sentence;
//! - [`Accessible::a11y_input`] — a text field: its label, its placeholder,
//!   and what is typed (never a masked secret's);
//! - [`Accessible::a11y_toggle`] — a switch, on or off;
//! - [`Accessible::a11y_dialog`] — a sheet or dialog, by its title;
//! - [`label`] — a line of text worth reading on its own (a total, a heading,
//!   a verdict), as a Label node.
//!
//! An icon-only control is labelled by what it does, never by its glyph.

use gpui::{
    Div, ElementId, InteractiveElement as _, ParentElement as _, Role, SharedString, Stateful,
    StatefulInteractiveElement, Toggled, div,
};

/// The roles and names this app gives its elements.
pub trait Accessible: StatefulInteractiveElement + Sized {
    /// A control that is pressed, named by what it says.
    fn a11y_button(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::Button).aria_label(label)
    }

    /// A tab-like section switch, and whether it is the one shown.
    fn a11y_tab(self, label: impl Into<SharedString>, selected: bool) -> Self {
        self.role(Role::Tab)
            .aria_label(label)
            .aria_selected(selected)
    }

    /// One row of a list, read as one sentence.
    fn a11y_item(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::ListItem).aria_label(label)
    }

    /// A text field: what it is for, its hint, and — unless it holds a
    /// secret — what is typed in it.
    fn a11y_input(
        self,
        label: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        value: Option<&str>,
    ) -> Self {
        let field = self
            .role(Role::TextInput)
            .aria_label(label)
            .aria_placeholder(placeholder);
        match value {
            Some(value) => field.aria_value(value.to_owned()),
            None => field,
        }
    }

    /// A switch, on or off.
    fn a11y_toggle(self, label: impl Into<SharedString>, on: bool) -> Self {
        self.role(Role::Switch)
            .aria_label(label)
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
    }

    /// A dialog or sheet, named by its title.
    fn a11y_dialog(self, title: impl Into<SharedString>) -> Self {
        self.role(Role::Dialog).aria_label(title)
    }
}

impl<E: StatefulInteractiveElement + Sized> Accessible for E {}

/// `VELA_A11Y_DUMP=<path>` (developer builds): the accessibility tree of the
/// last frame, as gpui reports it (`Window::debug_a11y_tree_json`), written to
/// `path` whenever it changes — the snapshot a UI driver (T4) or a review
/// diffs. gpui builds the tree only while an assistive client is attached
/// (VoiceOver, Accessibility Inspector, Narrator, Orca), so start one first;
/// the test platform never attaches one, which is why this is a probe on a
/// real window and not a unit test.
pub fn dump_tree(window: &gpui::Window) {
    let Some(path) = crate::dev_env::var!("VELA_A11Y_DUMP") else {
        return;
    };
    let Some(json) = window.debug_a11y_tree_json() else {
        return;
    };
    static LAST: std::sync::Mutex<Option<u64>> = std::sync::Mutex::new(None);
    let digest = {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        json.hash(&mut hasher);
        hasher.finish()
    };
    if let Ok(mut last) = LAST.lock()
        && *last != Some(digest)
        && std::fs::write(&path, json).is_ok()
    {
        *last = Some(digest);
    }
}

/// A screen's or a column's title, as a Heading node — what VoiceOver's
/// rotor jumps between.
pub fn heading(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();
    div()
        .id(id)
        .role(Role::Heading)
        .aria_label(text.clone())
        .child(text)
}

/// "Close", in the app's language — the name of every ✕ that carries no word
/// of its own (a dialog's, a column's). Resolved once per language: a ✕ is
/// drawn every frame, an engine is not built every frame.
pub fn close_label() -> SharedString {
    static CACHE: std::sync::Mutex<Option<(String, SharedString)>> = std::sync::Mutex::new(None);
    let language = crate::loc::app_language();
    let Ok(mut cache) = CACHE.lock() else {
        return SharedString::from("Close");
    };
    if let Some((cached, word)) = cache.as_ref()
        && *cached == language
    {
        return word.clone();
    }
    let word = crate::loc::Loc::for_language(&language).t("componentsUi.signing.close");
    *cache = Some((language, word.clone()));
    word
}

/// The words a list row is read as: its parts joined, the empty ones left out
/// (a contact with no saved name reads as its address alone).
#[must_use]
pub fn sentence<'a>(parts: impl IntoIterator<Item = &'a str>) -> SharedString {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_reads_as_its_parts_without_the_empty_ones() {
        assert_eq!(
            sentence(["USDC", " ", "Gnosis", "$10.00"]).as_ref(),
            "USDC, Gnosis, $10.00"
        );
        assert_eq!(sentence(["", "0xabc"]).as_ref(), "0xabc");
        assert_eq!(sentence([]).as_ref(), "");
    }
}
