//! Reusable visual components. Everything here takes `&Theme` plus resolved
//! strings — no i18n, no page state, no window management.

mod ack_row;
mod button;
pub mod dialog;
mod launch_animation;
mod logo;
mod name_field;
mod rail;
mod scrollbar;
mod smooth_scroll;
mod spinner;
mod status_badge;

pub use ack_row::ack_row;
pub use button::{
    ButtonState, ButtonVariant, vela_button, vela_button_opts, vela_button_state, welcome_cta,
    welcome_cta_state,
};
pub use launch_animation::LaunchAnimation;
pub use logo::{vela_mark, vela_wordmark};
pub use name_field::{
    EditChord, NameFieldStrings, bare_text_field, edit_chord, hero_amount_field, name_field,
    search_input, text_area, text_field,
};
pub use rail::{RailSlot, onboarding_rail};
pub use scrollbar::vertical_scrollbar;
pub use smooth_scroll::SmoothScroll;
pub use spinner::spinner;
pub use status_badge::status_badge;
