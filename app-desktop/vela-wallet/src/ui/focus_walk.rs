//! Tab and ⇧Tab between the window's tab stops (R22 follow-up r22-5, S3).
//!
//! gpui walks focus only when asked (`Window::focus_next` /
//! `focus_prev`); nothing in this shell asked, so a control drawn as a tab
//! stop — the held confirm (`ui::hold_confirm`), whose keyboard path is
//! Enter / Space, "Press again to confirm" — could never be reached from the
//! keyboard. The window's root now carries [`TAB_WALK`] and answers
//! `app_menu::FocusNext` / `FocusPrev`, which `app_menu` binds to Tab and
//! ⇧Tab under that context.
//!
//! A form that walks its own fields in its own order (the contact form's
//! name → address → note, the feedback form's wells and tiles) sets
//! [`OWN_TAB`] on its container: the binding stands aside there and the
//! form's own Tab handler runs exactly as before.

use gpui::InteractiveElement;

/// The key context the window's root sets for the focus walk.
pub const TAB_WALK: &str = "VelaTabWalk";

/// The key context of a form that walks its own fields on Tab.
pub const OWN_TAB: &str = "VelaOwnTab";

/// The binding's context: under the root's walk, and not inside a form that
/// walks its own fields.
pub const WALK_PREDICATE: &str = "VelaTabWalk && !VelaOwnTab";

/// The window's root, walking focus on Tab / ⇧Tab.
pub fn tab_walk<E: InteractiveElement>(root: E) -> E {
    root.key_context(TAB_WALK)
        .on_action(|_: &crate::app_menu::FocusNext, window, cx| window.focus_next(cx))
        .on_action(|_: &crate::app_menu::FocusPrev, window, cx| window.focus_prev(cx))
}
