//! Reusable visual components. Everything here takes `&Theme` plus resolved
//! strings — no i18n, no page state, no window management.

pub mod a11y;
mod ack_row;
mod address;
mod button;
pub mod dialog;
pub mod editor;
pub mod editor_model;
pub mod hold_confirm;
mod launch_animation;
mod logo;
pub mod motion;
mod name_field;
pub mod prose;
mod rail;
mod scrollbar;
mod smooth_scroll;
mod spinner;
mod status_badge;
mod step_slider;
mod windowed;

pub use a11y::Accessible;
pub use ack_row::ack_row;
pub use address::{
    address_halves, address_text, breaks_a_full_address, breaks_a_short_form, full_address_spans,
    short_form_spans,
};
pub use button::{
    ButtonState, ButtonVariant, button_face, vela_button, vela_button_opts, vela_button_state,
    welcome_cta, welcome_cta_state,
};
pub use hold_confirm::{HoldWords, held_confirm, hold_fill};
pub use launch_animation::LaunchAnimation;
pub use logo::{vela_mark, vela_wordmark};
pub use name_field::{
    EditChord, NameFieldStrings, bare_text_field, edit_chord, hero_amount_field, name_field,
    search_input, text_area, text_field,
};
pub use prose::prose;
pub use rail::{RailSlot, onboarding_rail};
pub use scrollbar::vertical_scrollbar;
pub use smooth_scroll::SmoothScroll;
pub use spinner::spinner;
/// The badge's glyph, for the gallery's check that no failure wears a ×.
#[cfg(test)]
pub(crate) use status_badge::glyph as badge_glyph;
pub use status_badge::status_badge;
pub use step_slider::{StepSlider, step_slider_picture};
pub use windowed::Windowed;
