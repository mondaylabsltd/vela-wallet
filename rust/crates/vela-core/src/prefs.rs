//! The person's display preferences as every shell stores them (spec 072):
//! the key names, the value vocabularies, the defaults, and how an older or
//! foreign spelling reads.
//!
//! Four shells wrote the same five preferences four ways. Android stored
//! `system` where the others store `auto` for "follow the device's language"
//! and kept the text size inside `vela.localePrefs`; its theme lived outside
//! the key-value store altogether; the desktop wrote `vela.formats` with its
//! own field names and did not store the rest. A record written by one shell
//! then meant something else, or nothing, to the others. The vocabulary is
//! the web's (`preferences.svelte.ts`, itself the Expo record format), and
//! this module is now the one place it is written down.
//!
//! Pure: the shell reads its store, hands the entries here, and writes back
//! what [`migrations`] says.

use serde_json::{json, Map, Value};

/// The keys — the record format every shell shares.
pub mod keys {
    pub const THEME: &str = "vela.theme";
    pub const LANGUAGE: &str = "vela.language";
    pub const LOCALE_PREFS: &str = "vela.localePrefs";
    pub const AVATAR_STYLE: &str = "vela.avatarStyle";
    pub const TEXT_SCALE: &str = "vela.textScale";
    /// The desktop's old spelling of `vela.localePrefs` (`{number,date,time}`).
    pub const LEGACY_FORMATS: &str = "vela.formats";
}

/// `system` follows the device; the others pin it.
pub const THEMES: [&str; 3] = ["system", "light", "dark"];
pub const AVATAR_STYLES: [&str; 2] = ["initials", "identicon"];
pub const NUMBER_FORMATS: [&str; 5] = ["auto", "comma_dot", "dot_comma", "space_comma", "indian"];
pub const DATE_FORMATS: [&str; 6] = [
    "auto",
    "ymd_slash",
    "mdy_slash",
    "dmy_slash",
    "dmy_dot",
    "iso",
];
pub const TIME_FORMATS: [&str; 3] = ["auto", "h24", "h12"];

/// The six stops of the text-size slider and the factor each multiplies every
/// text token by (0.82–1.35, design-tokens `textScale`).
pub const TEXT_SCALE_LEVELS: [(&str, f64); 6] = [
    ("compact", 0.82),
    ("small", 0.91),
    ("standard", 1.0),
    ("comfortable", 1.1),
    ("large", 1.22),
    ("xlarge", 1.35),
];

pub const DEFAULT_THEME: &str = "system";
pub const DEFAULT_AVATAR_STYLE: &str = "identicon";
pub const DEFAULT_TEXT_SCALE: &str = "standard";
/// "Follow the device's language."
pub const AUTO_LANGUAGE: &str = "auto";

/// What the preferences are, whatever spelling the store holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub theme: &'static str,
    /// `auto`, or a locale tag as stored.
    pub language: String,
    pub avatar_style: &'static str,
    pub text_scale: &'static str,
    pub number_format: &'static str,
    pub date_format: &'static str,
    pub time_format: &'static str,
}

fn one_of(raw: Option<&str>, allowed: &[&'static str], fallback: &'static str) -> &'static str {
    let raw = raw.map(str::trim).unwrap_or_default();
    allowed
        .iter()
        .copied()
        .find(|value| *value == raw)
        .unwrap_or(fallback)
}

/// A stored theme: `auto` (Android's word) reads as `system`.
#[must_use]
pub fn theme(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim) {
        Some("auto") => "system",
        other => one_of(other, &THEMES, DEFAULT_THEME),
    }
}

/// A stored language: `system` (Android's word), empty and absent read as
/// [`AUTO_LANGUAGE`].
#[must_use]
pub fn language(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        None | Some("" | "system" | "auto") => AUTO_LANGUAGE.to_owned(),
        Some(tag) => tag.to_owned(),
    }
}

/// A stored text size: a level's name, or an older shell's slider index.
#[must_use]
pub fn text_scale(raw: Option<&str>) -> &'static str {
    let raw = raw.map(str::trim).unwrap_or_default();
    if let Ok(index) = raw.parse::<usize>() {
        if let Some((name, _)) = TEXT_SCALE_LEVELS.get(index) {
            return name;
        }
    }
    TEXT_SCALE_LEVELS
        .iter()
        .map(|(name, _)| *name)
        .find(|name| *name == raw)
        .unwrap_or(DEFAULT_TEXT_SCALE)
}

/// The factor a level multiplies text by.
#[must_use]
pub fn text_scale_factor(level: &str) -> f64 {
    TEXT_SCALE_LEVELS
        .iter()
        .find(|(name, _)| *name == level)
        .map_or(1.0, |(_, factor)| *factor)
}

/// Read every preference from the store's entries (`key → raw value`).
#[must_use]
pub fn read(entries: &[(String, String)]) -> Prefs {
    let get = |key: &str| {
        entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let locale = get(keys::LOCALE_PREFS)
        .or_else(|| get(keys::LEGACY_FORMATS))
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or(Value::Null);
    let field = |canonical: &str, legacy: &str| {
        locale
            .get(canonical)
            .or_else(|| locale.get(legacy))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let scale = get(keys::TEXT_SCALE).map(str::to_owned).or_else(|| {
        locale.get("textScale").map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
    });
    Prefs {
        theme: theme(get(keys::THEME)),
        language: language(get(keys::LANGUAGE)),
        avatar_style: one_of(
            get(keys::AVATAR_STYLE),
            &AVATAR_STYLES,
            DEFAULT_AVATAR_STYLE,
        ),
        text_scale: text_scale(scale.as_deref()),
        number_format: one_of(
            field("numberFormat", "number").as_deref(),
            &NUMBER_FORMATS,
            "auto",
        ),
        date_format: one_of(
            field("dateFormat", "date").as_deref(),
            &DATE_FORMATS,
            "auto",
        ),
        time_format: one_of(
            field("timeFormat", "time").as_deref(),
            &TIME_FORMATS,
            "auto",
        ),
    }
}

/// The `vela.localePrefs` record for three formats.
#[must_use]
pub fn locale_prefs_json(number_format: &str, date_format: &str, time_format: &str) -> String {
    json!({ "numberFormat": number_format, "dateFormat": date_format, "timeFormat": time_format })
        .to_string()
}

/// What to write (`Some`) or remove (`None`) so the store holds the shared
/// spelling of what an older shell wrote. Only the KNOWN legacy spellings are
/// rewritten — a value this build does not ship reads as the default but is
/// left alone, so a newer build's choice survives a trip through this one.
/// Empty for a store that already agrees, so it is safe at every launch.
#[must_use]
pub fn migrations(entries: &[(String, String)]) -> Vec<(String, Option<String>)> {
    let get = |key: &str| {
        entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim())
    };
    let prefs = read(entries);
    let mut writes = Vec::new();
    if get(keys::THEME) == Some("auto") {
        writes.push((keys::THEME.to_owned(), Some(prefs.theme.to_owned())));
    }
    if matches!(get(keys::LANGUAGE), Some("system" | "")) {
        writes.push((keys::LANGUAGE.to_owned(), Some(AUTO_LANGUAGE.to_owned())));
    }
    // Android kept the text size inside `vela.localePrefs`: it moves to its
    // own key, and the record is written back without it.
    let stored_locale = get(keys::LOCALE_PREFS)
        .and_then(|raw| serde_json::from_str::<Map<String, Value>>(raw).ok());
    let nested_scale = stored_locale
        .as_ref()
        .is_some_and(|map| map.contains_key("textScale"));
    if nested_scale && get(keys::TEXT_SCALE).is_none() {
        writes.push((
            keys::TEXT_SCALE.to_owned(),
            Some(prefs.text_scale.to_owned()),
        ));
    }
    // The desktop's `vela.formats` becomes the shared record.
    let legacy = get(keys::LEGACY_FORMATS).is_some();
    if nested_scale || (legacy && stored_locale.is_none()) {
        writes.push((
            keys::LOCALE_PREFS.to_owned(),
            Some(locale_prefs_json(
                prefs.number_format,
                prefs.date_format,
                prefs.time_format,
            )),
        ));
    }
    if legacy {
        writes.push((keys::LEGACY_FORMATS.to_owned(), None));
    }
    writes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn nothing_stored_is_every_default() {
        let prefs = read(&[]);
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.language, "auto");
        assert_eq!(prefs.avatar_style, "identicon");
        assert_eq!(prefs.text_scale, "standard");
        assert_eq!(
            (prefs.number_format, prefs.date_format, prefs.time_format),
            ("auto", "auto", "auto")
        );
        assert!(migrations(&[]).is_empty());
    }

    #[test]
    fn androids_spellings_read_as_the_shared_ones_and_are_rewritten() {
        let stored = entries(&[
            ("vela.theme", "auto"),
            ("vela.language", "system"),
            (
                "vela.localePrefs",
                r#"{"numberFormat":"space_comma","dateFormat":"ymd_slash","timeFormat":"h24","textScale":4}"#,
            ),
        ]);
        let prefs = read(&stored);
        assert_eq!(
            (prefs.theme, prefs.language.as_str(), prefs.text_scale),
            ("system", "auto", "large")
        );
        assert_eq!(prefs.number_format, "space_comma");
        let writes = migrations(&stored);
        assert!(writes.contains(&("vela.theme".into(), Some("system".into()))));
        assert!(writes.contains(&("vela.language".into(), Some("auto".into()))));
        assert!(writes.contains(&("vela.textScale".into(), Some("large".into()))));
        assert!(writes.contains(&(
            "vela.localePrefs".into(),
            Some(
                r#"{"dateFormat":"ymd_slash","numberFormat":"space_comma","timeFormat":"h24"}"#
                    .into()
            )
        )));
        // Once rewritten, nothing more to do.
        let after: Vec<(String, String)> = writes
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect();
        assert!(migrations(&after).is_empty());
    }

    #[test]
    fn the_desktops_formats_record_becomes_the_shared_one() {
        let stored = entries(&[(
            "vela.formats",
            r#"{"number":"dot_comma","date":"iso","time":"h12"}"#,
        )]);
        let prefs = read(&stored);
        assert_eq!(
            (prefs.number_format, prefs.date_format, prefs.time_format),
            ("dot_comma", "iso", "h12")
        );
        let writes = migrations(&stored);
        assert_eq!(writes.last(), Some(&("vela.formats".into(), None)));
        assert!(writes.iter().any(|(k, v)| k == "vela.localePrefs"
            && v.as_deref().is_some_and(|v| v.contains("dot_comma"))));
    }

    #[test]
    fn a_value_this_build_does_not_ship_reads_as_the_default_and_is_left_alone() {
        let stored = entries(&[
            ("vela.theme", "sepia"),
            ("vela.textScale", "huge"),
            ("vela.avatarStyle", "photo"),
        ]);
        assert!(
            migrations(&stored).is_empty(),
            "a newer build's value is not overwritten"
        );
        let prefs = read(&stored);
        assert_eq!(
            (prefs.theme, prefs.text_scale, prefs.avatar_style),
            ("system", "standard", "identicon")
        );
        assert_eq!(text_scale_factor("xlarge"), 1.35);
        assert_eq!(
            read(&entries(&[("vela.language", "zh-TW")])).language,
            "zh-TW"
        );
    }
}
