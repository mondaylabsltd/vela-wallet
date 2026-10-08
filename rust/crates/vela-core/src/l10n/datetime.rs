//! Dates, times, weekday names and day periods.
//!
//! **The core has no timezone database and never will.** The host supplies the UTC
//! offset it would otherwise have got from `Date.prototype.getHours()`, and
//! everything here is pure arithmetic on that. A tz database is I/O-shaped, large,
//! and would need updating on a schedule this crate cannot honour.
//!
//! Presets, not `Intl` — same rule as `l10n::number`. The day-period and weekday
//! strings come from the generated [`super::datetime_data`] table, which closes the
//! last two host-`Intl` dependencies on the formatting path: the hardcoded English
//! `AM`/`PM` in `locale-format.ts` (wrong in *wording* for 8 of the 15 shipped
//! locales and in *position* for 6) and `activity.ts:121`'s `toLocaleDateString`
//! weekday lookup.

use super::datetime_data;
use core::fmt::Write as _;

/// Date layout presets, matching `DateFormatKey` in `src/models/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatePreset {
    /// `2026/06/13`
    YmdSlash,
    /// `06/13/2026`
    #[default]
    MdySlash,
    /// `13/06/2026`
    DmySlash,
    /// `13.06.2026`
    DmyDot,
    /// `2026-06-13`
    Iso,
}

/// The preset a shell names on the wire — the stored word
/// (`prefs::DATE_FORMATS`): `ymd_slash`, `mdy_slash`, `dmy_slash`, `dmy_dot`,
/// `iso`. `auto` and anything unknown read as the default, `mdy_slash`: the
/// shells resolve `auto` from the device before they call, the same as
/// `amount_text::preset_of` for numbers.
#[must_use]
pub fn date_preset_of(key: &str) -> DatePreset {
    match key {
        "ymd_slash" => DatePreset::YmdSlash,
        "dmy_slash" => DatePreset::DmySlash,
        "dmy_dot" => DatePreset::DmyDot,
        "iso" => DatePreset::Iso,
        _ => DatePreset::MdySlash,
    }
}

/// Clock presets, matching `TimeFormatKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimePreset {
    /// `13:45`
    #[default]
    H24,
    /// `1:45 PM` — unpadded hour, exactly one U+0020 beside the marker.
    H12,
}

/// A wall-clock date and time, already shifted into the caller's zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    /// 1..=12
    pub month: u32,
    /// 1..=31
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// 0 = Sunday, matching JS `Date.prototype.getDay()`.
    pub weekday: u32,
}

impl Civil {
    /// Convert a Unix millisecond timestamp into wall-clock fields.
    ///
    /// `utc_offset_minutes` is what the host would apply — e.g. `-420` for UTC-7.
    /// Pure arithmetic (Howard Hinnant's `civil_from_days`): exact over the whole
    /// proleptic Gregorian range, no lookup tables, no allocation.
    #[must_use]
    pub fn from_unix_millis(ms: i64, utc_offset_minutes: i32) -> Self {
        let shifted = ms.saturating_add(i64::from(utc_offset_minutes) * 60_000);
        // FLOOR division, not truncating: a pre-1970 timestamp must round down, or
        // 1969-12-31 23:00 lands on 1970-01-01.
        let days = shifted.div_euclid(86_400_000);
        let rem_ms = shifted.rem_euclid(86_400_000);

        let (year, month, day) = civil_from_days(days);
        // 1970-01-01 was a Thursday (4).
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::allow_attributes
        )]
        let weekday = (days + 4).rem_euclid(7) as u32;

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::allow_attributes
        )]
        Self {
            year,
            month,
            day,
            hour: (rem_ms / 3_600_000) as u32,
            minute: (rem_ms / 60_000 % 60) as u32,
            second: (rem_ms / 1_000 % 60) as u32,
            weekday,
        }
    }

    /// Whole days between two civil dates, ignoring the time of day. This is what
    /// the today/yesterday rule needs — "yesterday" is a calendar relation, not a
    /// 24-hour one.
    #[must_use]
    pub fn days_between(&self, other: &Civil) -> i64 {
        days_from_civil(self.year, self.month, self.day)
            - days_from_civil(other.year, other.month, other.day)
    }
}

/// Days since 1970-01-01 for a civil date.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let m = i64::from(m);
    let d = i64::from(d);
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::allow_attributes
    )]
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn pad2(out: &mut String, n: u32) {
    if n < 10 {
        out.push('0');
    }
    let _ = write!(out, "{n}");
}

/// Format a date with the chosen preset.
#[must_use]
pub fn format_date(civil: &Civil, preset: DatePreset) -> String {
    let mut out = String::with_capacity(10);
    match preset {
        DatePreset::YmdSlash => {
            let _ = write!(out, "{}/", civil.year);
            pad2(&mut out, civil.month);
            out.push('/');
            pad2(&mut out, civil.day);
        }
        DatePreset::Iso => {
            let _ = write!(out, "{}-", civil.year);
            pad2(&mut out, civil.month);
            out.push('-');
            pad2(&mut out, civil.day);
        }
        DatePreset::DmySlash | DatePreset::DmyDot => {
            let sep = if matches!(preset, DatePreset::DmyDot) {
                '.'
            } else {
                '/'
            };
            pad2(&mut out, civil.day);
            out.push(sep);
            pad2(&mut out, civil.month);
            out.push(sep);
            let _ = write!(out, "{}", civil.year);
        }
        DatePreset::MdySlash => {
            pad2(&mut out, civil.month);
            out.push('/');
            pad2(&mut out, civil.day);
            out.push('/');
            let _ = write!(out, "{}", civil.year);
        }
    }
    out
}

/// Format a time with the chosen preset, in `locale`'s day-period convention.
///
/// The `h12` form is where the old implementation was wrong twice over: it
/// hardcoded English `AM`/`PM`, and it always appended them. Six of the fifteen
/// shipped locales write the marker **before** the hour (`午後 1:45`), and eight
/// use different text entirely (`p.m.`, `ÖS`, `SA`).
#[must_use]
pub fn format_time(civil: &Civil, preset: TimePreset, locale: &str) -> String {
    let mut out = String::with_capacity(12);
    match preset {
        TimePreset::H24 => {
            pad2(&mut out, civil.hour);
            out.push(':');
            pad2(&mut out, civil.minute);
        }
        TimePreset::H12 => {
            let row = datetime_data::row(locale);
            let marker = if civil.hour < 12 { row.1 } else { row.2 };
            let period_first = row.3;
            let h12 = if civil.hour % 12 == 0 {
                12
            } else {
                civil.hour % 12
            };
            if period_first {
                out.push_str(marker);
                out.push(' ');
            }
            let _ = write!(out, "{h12}:");
            pad2(&mut out, civil.minute);
            if !period_first {
                out.push(' ');
                out.push_str(marker);
            }
        }
    }
    out
}

/// `"<date>, <time>"` — a comma then exactly one U+0020.
#[must_use]
pub fn format_date_time(civil: &Civil, date: DatePreset, time: TimePreset, locale: &str) -> String {
    format!(
        "{}, {}",
        format_date(civil, date),
        format_time(civil, time, locale)
    )
}

/// The short weekday name for `civil` in `locale`. Index 0 is Sunday.
#[must_use]
pub fn weekday_name(civil: &Civil, locale: &str) -> &'static str {
    let row = datetime_data::row(locale);
    row.4[(civil.weekday % 7) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-06-13 13:45 UTC — the sample `locale-format.ts` documents, chosen
    /// because it disambiguates every date order.
    fn sample() -> Civil {
        Civil::from_unix_millis(1_781_358_300_000, 0)
    }

    #[test]
    fn civil_conversion_round_trips_the_documented_sample() {
        let c = sample();
        assert_eq!((c.year, c.month, c.day), (2026, 6, 13));
        assert_eq!((c.hour, c.minute), (13, 45));
        assert_eq!(c.weekday, 6, "2026-06-13 is a Saturday");
    }

    #[test]
    fn date_presets_match_locale_format_ts() {
        let c = sample();
        assert_eq!(format_date(&c, DatePreset::YmdSlash), "2026/06/13");
        assert_eq!(format_date(&c, DatePreset::Iso), "2026-06-13");
        assert_eq!(format_date(&c, DatePreset::DmySlash), "13/06/2026");
        assert_eq!(format_date(&c, DatePreset::DmyDot), "13.06.2026");
        assert_eq!(format_date(&c, DatePreset::MdySlash), "06/13/2026");
    }

    #[test]
    fn a_stored_date_word_names_its_preset() {
        let c = sample();
        let shown = |word: &str| format_date(&c, date_preset_of(word));
        assert_eq!(shown("ymd_slash"), "2026/06/13");
        assert_eq!(shown("mdy_slash"), "06/13/2026");
        assert_eq!(shown("dmy_slash"), "13/06/2026");
        assert_eq!(shown("dmy_dot"), "13.06.2026");
        assert_eq!(shown("iso"), "2026-06-13");
        // `auto` is the shell's to resolve; unresolved, it is the default.
        assert_eq!(date_preset_of("auto"), DatePreset::default());
        assert_eq!(date_preset_of("YMD_SLASH"), DatePreset::MdySlash);
    }

    #[test]
    fn h24_matches_and_h12_localises_the_day_period() {
        let c = sample();
        assert_eq!(format_time(&c, TimePreset::H24, "en"), "13:45");
        // The old behaviour, preserved for English.
        assert_eq!(format_time(&c, TimePreset::H12, "en"), "1:45 PM");
        // Four of the eight locales the hardcoded English suffix got wrong.
        assert_eq!(format_time(&c, TimePreset::H12, "ja"), "午後 1:45");
        assert_eq!(format_time(&c, TimePreset::H12, "es-MX"), "1:45 p.m.");
        assert_eq!(format_time(&c, TimePreset::H12, "tr"), "ÖS 1:45");
        assert_eq!(format_time(&c, TimePreset::H12, "vi"), "1:45 CH");
    }

    #[test]
    fn midnight_and_noon_use_twelve_not_zero() {
        let midnight = Civil::from_unix_millis(1_781_308_800_000, 0);
        assert_eq!(midnight.hour, 0);
        assert_eq!(format_time(&midnight, TimePreset::H12, "en"), "12:00 AM");
        assert_eq!(format_time(&midnight, TimePreset::H24, "en"), "00:00");
    }

    #[test]
    fn combined_form_uses_comma_space() {
        let c = sample();
        assert_eq!(
            format_date_time(&c, DatePreset::YmdSlash, TimePreset::H24, "en"),
            "2026/06/13, 13:45"
        );
    }

    #[test]
    fn weekday_names_come_from_the_generated_table() {
        let c = sample();
        assert_eq!(weekday_name(&c, "en"), "Sat");
        assert_eq!(weekday_name(&c, "ja"), "土");
        assert_eq!(weekday_name(&c, "ru"), "сб");
        // An unknown locale falls back to `en`, like the resolver does.
        assert_eq!(weekday_name(&c, "xx"), "Sat");
    }

    #[test]
    fn offsets_shift_the_wall_clock_without_a_tz_database() {
        // UTC-7: 13:45 UTC is 06:45 local, same day.
        let c = Civil::from_unix_millis(1_781_358_300_000, -420);
        assert_eq!((c.hour, c.day), (6, 13));
        // UTC+14 crosses midnight into the next day.
        let c = Civil::from_unix_millis(1_781_358_300_000, 840);
        assert_eq!((c.hour, c.day), (3, 14));
    }

    #[test]
    fn pre_epoch_timestamps_floor_rather_than_truncate() {
        // 1969-12-31 23:00 UTC. Truncating division would land on 1970-01-01.
        let c = Civil::from_unix_millis(-3_600_000, 0);
        assert_eq!((c.year, c.month, c.day, c.hour), (1969, 12, 31, 23));
    }

    #[test]
    fn days_between_ignores_the_time_of_day() {
        let a = Civil::from_unix_millis(1_781_358_300_000, 0); // 13th 13:45
        let b = Civil::from_unix_millis(1_781_308_800_000, 0); // 13th 00:00
        assert_eq!(a.days_between(&b), 0);
        let c = Civil::from_unix_millis(1_781_222_400_000, 0); // 12th
        assert_eq!(a.days_between(&c), 1);
    }
}
