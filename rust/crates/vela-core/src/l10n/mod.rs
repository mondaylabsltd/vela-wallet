//! Locale-aware formatting: numbers, dates, times, currency, bidi.
//!
//! **Phase 2 skeleton** — Phase 6 (tasks T050–T059) implements this.
//!
//! Two rules govern the whole module:
//!
//! 1. **No host `Intl`, ever.** `src/services/locale-format.ts` avoids it because
//!    Hermes ships incomplete ICU, and that judgement is correct — it is also why
//!    ten Russian plural keys render wrongly on every native build today. The Rust
//!    engine carries its own data; it must never consult a host locale facility.
//! 2. **Presets are a product decision, not an accident.** The user picks a number,
//!    date and time format explicitly in Settings. CLDR governs currency *fraction
//!    digits and symbol placement* (FR-020); it does **not** get to override the
//!    separators the user chose. The currency path composes the two.
pub mod amount_text;
pub mod bidi;
pub mod currency;
pub mod datetime;
mod datetime_data;
pub mod number;

pub use bidi::{isolate, isolate_into, text_direction};
pub use currency::{currency_fraction_digits, format_fiat, FiatOptions};
pub use datetime::{
    format_date, format_date_time, format_time, weekday_name, Civil, DatePreset, TimePreset,
};
pub use number::{
    format_compact, format_number, format_token_amount, group_digits, parse_locale_number,
    FractionDigits, NumberPreset, Separators,
};
