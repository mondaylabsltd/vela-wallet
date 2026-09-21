//! Number, date and clock presets — the person's choice from Settings →
//! Localization, or the machine's own conventions when they never chose.
//!
//! Spec 038 #E3: the desktop's three format rows were drawn but not wired, so
//! "13.06.2026" could be picked and change nothing. The web resolves
//! "Automatic" through `Intl`; a desktop has no `Intl`, so the same answer
//! comes from a small table over the locale tags the app ships (the way
//! `display_currency::region_currency` does for money). The choice is stored
//! as the record every Vela shares, `vela.localePrefs`, read through the
//! core's `prefs` (spec 072) — until then the desktop wrote its own
//! `vela.formats` with its own field names, which the launch migration now
//! moves over (`preferences::migrate`).

use std::sync::{Mutex, OnceLock, PoisonError};

use serde_json::Value;
use vela_core::l10n::currency::FiatOptions;
use vela_core::l10n::datetime::{DatePreset, TimePreset};
use vela_core::l10n::number::NumberPreset;
use vela_core::prefs::{self, Prefs};

use crate::executor::storage;

/// The store key: `{numberFormat, dateFormat, timeFormat}`, each a preset
/// word or `auto`.
pub const KEY: &str = prefs::keys::LOCALE_PREFS;

/// The pickable presets, in the order the web's sheets list them.
pub const NUMBER_OPTIONS: [NumberPreset; 4] = [
    NumberPreset::CommaDot,
    NumberPreset::DotComma,
    NumberPreset::SpaceComma,
    NumberPreset::Indian,
];
pub const DATE_OPTIONS: [DatePreset; 5] = [
    DatePreset::YmdSlash,
    DatePreset::MdySlash,
    DatePreset::DmySlash,
    DatePreset::DmyDot,
    DatePreset::Iso,
];
pub const TIME_OPTIONS: [TimePreset; 2] = [TimePreset::H24, TimePreset::H12];

/// What the person chose. `None` is "Automatic · System".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Choice {
    pub number: Option<NumberPreset>,
    pub date: Option<DatePreset>,
    pub time: Option<TimePreset>,
}

/// The presets every figure, date and clock on screen is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formats {
    pub number: NumberPreset,
    pub date: DatePreset,
    pub time: TimePreset,
}

static CHOICE: OnceLock<Mutex<Choice>> = OnceLock::new();

fn cell() -> &'static Mutex<Choice> {
    CHOICE.get_or_init(|| Mutex::new(load()))
}

/// The stored choice, read once per launch.
#[must_use]
pub fn choice() -> Choice {
    *cell().lock().unwrap_or_else(PoisonError::into_inner)
}

/// The machine's own conventions, from the same locale ladder the strings use.
#[must_use]
pub fn machine() -> Formats {
    auto(&device_tag())
}

/// The presets in force: the choice where one was made, the machine's
/// conventions where it was not.
#[must_use]
pub fn current() -> Formats {
    resolve(choice(), &machine())
}

/// The fiat options every fiat figure is drawn with: the product's
/// drop-minor-units rule kept, the separators the person's.
#[must_use]
pub fn fiat_options() -> FiatOptions {
    FiatOptions {
        preset: current().number,
        ..FiatOptions::default()
    }
}

pub fn set_number(preset: Option<NumberPreset>) {
    update(|choice| choice.number = preset);
}

pub fn set_date(preset: Option<DatePreset>) {
    update(|choice| choice.date = preset);
}

pub fn set_time(preset: Option<TimePreset>) {
    update(|choice| choice.time = preset);
}

fn update(apply: impl FnOnce(&mut Choice)) {
    let mut guard = cell().lock().unwrap_or_else(PoisonError::into_inner);
    apply(&mut guard);
    // Best effort, as every client's preference write is: the in-memory choice
    // stays authoritative for this session either way.
    let _ = storage::write_value(KEY, encode(*guard));
}

fn load() -> Choice {
    decode(&prefs::read(&storage::raw_entries().unwrap_or_default()))
}

/// Read the store again — after an erase, which leaves "Automatic" behind.
pub fn reload() {
    *cell().lock().unwrap_or_else(PoisonError::into_inner) = load();
}

/// The tag the strings resolved from — pinned to `en` under test so a figure a
/// test pins does not depend on the machine running it.
fn device_tag() -> String {
    if cfg!(test) {
        return "en".to_owned();
    }
    static TAG: OnceLock<String> = OnceLock::new();
    TAG.get_or_init(crate::loc::system_tag).clone()
}

#[must_use]
pub fn resolve(choice: Choice, auto: &Formats) -> Formats {
    Formats {
        number: choice.number.unwrap_or(auto.number),
        date: choice.date.unwrap_or(auto.date),
        time: choice.time.unwrap_or(auto.time),
    }
}

/// The machine's conventions for a locale tag (`en-US`, `zh_CN`, `de`).
///
/// What `Intl` answers the web for the same tag, over the locales the app
/// ships and their neighbours; anything unlisted reads as the web's defaults.
#[must_use]
pub fn auto(tag: &str) -> Formats {
    let lower = tag.to_ascii_lowercase();
    let mut parts = lower.split(['-', '_', '.']);
    let lang = parts.next().unwrap_or("");
    let region = parts.next().unwrap_or("");
    let number = match lang {
        "de" | "id" | "vi" | "it" | "tr" | "pt" | "nl" | "da" | "el" => NumberPreset::DotComma,
        "es" if !matches!(region, "mx" | "us") => NumberPreset::DotComma,
        "fr" | "ru" | "pl" | "cs" | "sv" | "nb" | "fi" | "uk" => NumberPreset::SpaceComma,
        "hi" | "bn" | "mr" | "ta" | "te" | "gu" => NumberPreset::Indian,
        "en" if region == "in" => NumberPreset::Indian,
        _ => NumberPreset::CommaDot,
    };
    let date = match lang {
        "zh" | "ja" | "ko" | "hu" => DatePreset::YmdSlash,
        "sv" | "lt" => DatePreset::Iso,
        "de" | "ru" | "tr" | "fi" | "cs" | "pl" | "nb" | "da" | "uk" => DatePreset::DmyDot,
        "en" if matches!(region, "" | "us" | "ph") => DatePreset::MdySlash,
        _ => DatePreset::DmySlash,
    };
    let time = match lang {
        "en" if matches!(region, "" | "us" | "au" | "ca" | "in" | "nz" | "ph") => TimePreset::H12,
        "zh" if matches!(region, "tw" | "hk" | "mo") => TimePreset::H12,
        "es" if matches!(region, "mx" | "us") => TimePreset::H12,
        "ko" | "hi" | "ar" => TimePreset::H12,
        _ => TimePreset::H24,
    };
    Formats { number, date, time }
}

// The words the web stores (`NumberFormatKey` / `DateFormatKey` / `TimeFormatKey`).
const NUMBER_WORDS: [(NumberPreset, &str); 4] = [
    (NumberPreset::CommaDot, "comma_dot"),
    (NumberPreset::DotComma, "dot_comma"),
    (NumberPreset::SpaceComma, "space_comma"),
    (NumberPreset::Indian, "indian"),
];
const DATE_WORDS: [(DatePreset, &str); 5] = [
    (DatePreset::YmdSlash, "ymd_slash"),
    (DatePreset::MdySlash, "mdy_slash"),
    (DatePreset::DmySlash, "dmy_slash"),
    (DatePreset::DmyDot, "dmy_dot"),
    (DatePreset::Iso, "iso"),
];
const TIME_WORDS: [(TimePreset, &str); 2] = [(TimePreset::H24, "h24"), (TimePreset::H12, "h12")];

fn word<T: Copy + PartialEq>(table: &[(T, &'static str)], preset: Option<T>) -> &'static str {
    preset
        .and_then(|preset| table.iter().find(|(p, _)| *p == preset))
        .map_or("auto", |(_, word)| word)
}

fn preset<T: Copy>(table: &[(T, &'static str)], word: Option<&str>) -> Option<T> {
    let word = word?;
    table.iter().find(|(_, w)| *w == word).map(|(p, _)| *p)
}

/// The shared record, spelled by the core.
#[must_use]
pub fn encode(choice: Choice) -> Value {
    storage::stored_value(&prefs::locale_prefs_json(
        word(&NUMBER_WORDS, choice.number),
        word(&DATE_WORDS, choice.date),
        word(&TIME_WORDS, choice.time),
    ))
}

/// What the core read. Anything it could not — a missing field, a word this
/// build does not know — it already answered as `auto`, which is "Automatic"
/// here: never a crash and never somebody else's preset.
#[must_use]
pub fn decode(read: &Prefs) -> Choice {
    Choice {
        number: preset(&NUMBER_WORDS, Some(read.number_format)),
        date: preset(&DATE_WORDS, Some(read.date_format)),
        time: preset(&TIME_WORDS, Some(read.time_format)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the core reads out of one stored record.
    fn read_record(key: &str, record: &Value) -> Choice {
        decode(&prefs::read(&[(key.to_owned(), record.to_string())]))
    }

    #[test]
    fn the_web_s_words_round_trip() {
        for number in NUMBER_OPTIONS.map(Some).into_iter().chain([None]) {
            for date in DATE_OPTIONS.map(Some).into_iter().chain([None]) {
                for time in TIME_OPTIONS.map(Some).into_iter().chain([None]) {
                    let choice = Choice { number, date, time };
                    assert_eq!(read_record(KEY, &encode(choice)), choice);
                }
            }
        }
        // The web's record, field for field (spec 072) — not the desktop's
        // old `{number, date, time}`.
        assert_eq!(
            encode(Choice {
                number: None,
                date: Some(DatePreset::DmyDot),
                time: Some(TimePreset::H12)
            }),
            serde_json::json!({ "numberFormat": "auto", "dateFormat": "dmy_dot", "timeFormat": "h12" })
        );
    }

    #[test]
    fn an_unknown_word_reads_as_automatic() {
        let stored =
            serde_json::json!({ "numberFormat": "roman", "dateFormat": 13, "timeFormat": "h12" });
        assert_eq!(
            read_record(KEY, &stored),
            Choice {
                number: None,
                date: None,
                time: Some(TimePreset::H12)
            }
        );
        assert_eq!(decode(&prefs::read(&[])), Choice::default());
    }

    /// An older desktop's record still reads, before the launch migration has
    /// had its chance to move it.
    #[test]
    fn the_desktops_old_record_still_reads() {
        let old = serde_json::json!({ "number": "space_comma", "date": "iso", "time": "h24" });
        assert_eq!(
            read_record(prefs::keys::LEGACY_FORMATS, &old),
            Choice {
                number: Some(NumberPreset::SpaceComma),
                date: Some(DatePreset::Iso),
                time: Some(TimePreset::H24),
            }
        );
    }

    #[test]
    fn the_machine_s_conventions_match_what_intl_answers_the_web() {
        let cases = [
            (
                "en-US",
                NumberPreset::CommaDot,
                DatePreset::MdySlash,
                TimePreset::H12,
            ),
            (
                "en",
                NumberPreset::CommaDot,
                DatePreset::MdySlash,
                TimePreset::H12,
            ),
            (
                "en_GB.UTF-8",
                NumberPreset::CommaDot,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "en-IN",
                NumberPreset::Indian,
                DatePreset::DmySlash,
                TimePreset::H12,
            ),
            (
                "zh_CN.UTF-8",
                NumberPreset::CommaDot,
                DatePreset::YmdSlash,
                TimePreset::H24,
            ),
            (
                "zh-TW",
                NumberPreset::CommaDot,
                DatePreset::YmdSlash,
                TimePreset::H12,
            ),
            (
                "ja",
                NumberPreset::CommaDot,
                DatePreset::YmdSlash,
                TimePreset::H24,
            ),
            (
                "ko",
                NumberPreset::CommaDot,
                DatePreset::YmdSlash,
                TimePreset::H12,
            ),
            (
                "de-DE",
                NumberPreset::DotComma,
                DatePreset::DmyDot,
                TimePreset::H24,
            ),
            (
                "ru",
                NumberPreset::SpaceComma,
                DatePreset::DmyDot,
                TimePreset::H24,
            ),
            (
                "tr",
                NumberPreset::DotComma,
                DatePreset::DmyDot,
                TimePreset::H24,
            ),
            (
                "fr",
                NumberPreset::SpaceComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "es-MX",
                NumberPreset::CommaDot,
                DatePreset::DmySlash,
                TimePreset::H12,
            ),
            (
                "es-ES",
                NumberPreset::DotComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "pt-BR",
                NumberPreset::DotComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "it",
                NumberPreset::DotComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "vi",
                NumberPreset::DotComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
            (
                "id",
                NumberPreset::DotComma,
                DatePreset::DmySlash,
                TimePreset::H24,
            ),
        ];
        for (tag, number, date, time) in cases {
            assert_eq!(auto(tag), Formats { number, date, time }, "{tag}");
        }
    }

    #[test]
    fn a_choice_wins_over_the_machine_field_by_field() {
        let auto = auto("de-DE");
        let chosen = resolve(
            Choice {
                number: None,
                date: Some(DatePreset::Iso),
                time: Some(TimePreset::H12),
            },
            &auto,
        );
        assert_eq!(chosen.number, NumberPreset::DotComma);
        assert_eq!(chosen.date, DatePreset::Iso);
        assert_eq!(chosen.time, TimePreset::H12);
    }

    #[test]
    fn a_choice_survives_a_relaunch() {
        storage::tests::with_temp_state("formats", || {
            assert_eq!(load(), Choice::default());
            let choice = Choice {
                number: Some(NumberPreset::SpaceComma),
                date: Some(DatePreset::DmyDot),
                time: None,
            };
            storage::write_value(KEY, encode(choice)).expect("a preference write");
            assert_eq!(load(), choice);
        });
    }
}
