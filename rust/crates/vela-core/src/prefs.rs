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
    /// Retired (spec 074): every account and contact avatar is the
    /// identicon, so there is no style to choose. [`migrations`](super::migrations)
    /// removes what an older build stored here.
    pub const AVATAR_STYLE: &str = "vela.avatarStyle";
    pub const TEXT_SCALE: &str = "vela.textScale";
    /// The desktop's old spelling of `vela.localePrefs` (`{number,date,time}`).
    pub const LEGACY_FORMATS: &str = "vela.formats";
    /// Spec 075's cross-device pairing service, RETIRED on 2026-09-23 with the
    /// channel itself ("客户端支持回环 + 蓝牙就够了，不需要 websocket 隧道").
    /// Both spellings it ever had are removed by
    /// [`migrations`](super::migrations): no build reads them any more, and an
    /// address left behind for a service nobody runs is not the person's
    /// choice, it is litter.
    pub const RETIRED_CLEAR_SIGNER_TUNNEL: &str = "vela.clearSignerTunnel";
    /// The spelling that shipped before the 2026-09-23 relay → tunnel rename.
    pub const RETIRED_CLEAR_SIGNER_RELAY: &str = "vela.clearSignerRelay";
    /// The hidden developer switch (spec 091): `off` / `on` once revealed,
    /// absent until then. See [`DebugMode`](super::DebugMode).
    pub const DEBUG_MODE: &str = "vela.debugMode";
}

/// `system` follows the device; the others pin it.
pub const THEMES: [&str; 3] = ["system", "light", "dark"];
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
pub const DEFAULT_TEXT_SCALE: &str = "standard";
/// "Follow the device's language."
pub const AUTO_LANGUAGE: &str = "auto";

/// What the preferences are, whatever spelling the store holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub theme: &'static str,
    /// `auto`, or a locale tag as stored.
    pub language: String,
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

// ---------------------------------------------------------------------------
// Debug mode — the hidden developer switch (spec 091)
// ---------------------------------------------------------------------------

/// Settings' debug mode (spec 091; owner rulings 2026-10-02). With it on, the
/// in-app browsers offer the wallet to http pages on this device's own
/// network too (`app::dapp_permissions::offers_wallet`, the one rule).
///
/// **Developer builds only** — the builds that carry the parallel space
/// (Android `debug`, iOS `Debug`, the desktop's `dev-fixtures`). A store
/// build has no such mode: [`debug_mode`] reads it as hidden whatever is
/// stored, and [`version_tapped`] reveals nothing, so a release build offers
/// the wallet exactly as spec 088 does. The shell says which build it is and
/// nothing more.
///
/// In a developer build nobody meets it by accident either: the switch is
/// [`Hidden`](Self::Hidden) until the version in About is tapped
/// [`VERSION_TAPS`] times, and then stays in About, so it can be turned off
/// again. Stored under [`keys::DEBUG_MODE`] as `off` or `on`; absent — or any
/// other value — reads as hidden, and hidden is off. Erasing the device
/// removes the key, which hides the switch again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugMode {
    #[default]
    Hidden,
    Off,
    On,
}

impl DebugMode {
    /// What a stored value means.
    #[must_use]
    pub fn read(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some("on") => Self::On,
            Some("off") => Self::Off,
            _ => Self::Hidden,
        }
    }

    /// The name the bindings carry: `hidden`, `off` or `on`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Off => "off",
            Self::On => "on",
        }
    }

    /// A name back to the mode; anything else is hidden.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "on" => Self::On,
            "off" => Self::Off,
            _ => Self::Hidden,
        }
    }

    /// The switch, revealed, set to `on` — what is stored under
    /// [`keys::DEBUG_MODE`] is [`Self::stored`] of it.
    #[must_use]
    pub fn switched(on: bool) -> Self {
        if on {
            Self::On
        } else {
            Self::Off
        }
    }

    /// The value to store; `None` removes the key.
    #[must_use]
    pub fn stored(self) -> Option<&'static str> {
        match self {
            Self::Hidden => None,
            Self::Off => Some("off"),
            Self::On => Some("on"),
        }
    }

    /// Whether About shows the switch.
    #[must_use]
    pub fn revealed(self) -> bool {
        self != Self::Hidden
    }

    /// Whether debug mode is in force.
    #[must_use]
    pub fn is_on(self) -> bool {
        self == Self::On
    }
}

/// Settings' debug mode in force on this build (spec 091): what is stored
/// under [`keys::DEBUG_MODE`] in a `developer_build`; hidden — and so off — in
/// any other, whatever is stored. The only reading of the key there is: a
/// store build never acts on a value a developer build left behind.
#[must_use]
pub fn debug_mode(entries: &[(String, String)], developer_build: bool) -> DebugMode {
    if !developer_build {
        return DebugMode::Hidden;
    }
    DebugMode::read(
        entries
            .iter()
            .find(|(key, _)| key == keys::DEBUG_MODE)
            .map(|(_, value)| value.as_str()),
    )
}

/// Taps on About's version that reveal the switch — Android's developer
/// options count the same.
pub const VERSION_TAPS: u32 = 7;
/// A tap later than this after the one before starts the count again.
pub const VERSION_TAP_GAP_MS: f64 = 1_000.0;

/// The count so far — the shell keeps it for as long as About is open and
/// hands it back with every tap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VersionTaps {
    pub count: u32,
    pub last_ms: f64,
}

/// One tap on the version in About, at `now_ms`. Returns the count to keep
/// and whether this tap revealed the switch — the shell then stores
/// [`DebugMode::Off`] and says so once.
///
/// Taps count only in a run, each within [`VERSION_TAP_GAP_MS`] of the one
/// before; a slower tap, or a clock that went backwards, starts again at one.
/// Once the switch is revealed nothing counts — and outside a
/// `developer_build` nothing ever does.
#[must_use]
pub fn version_tapped(
    taps: VersionTaps,
    now_ms: f64,
    mode: DebugMode,
    developer_build: bool,
) -> (VersionTaps, bool) {
    if !developer_build || mode.revealed() {
        return (VersionTaps::default(), false);
    }
    let since = now_ms - taps.last_ms;
    let in_run = taps.count > 0 && (0.0..=VERSION_TAP_GAP_MS).contains(&since);
    let count = if in_run { taps.count + 1 } else { 1 };
    if count >= VERSION_TAPS {
        return (VersionTaps::default(), true);
    }
    (
        VersionTaps {
            count,
            last_ms: now_ms,
        },
        false,
    )
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
/// A retired key ([`keys::AVATAR_STYLE`], and both spellings of the Clear
/// Signer's pairing service) is removed whatever it holds.
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
    // The avatar style was a choice between initials and the identicon;
    // the identicon is now the only avatar, and the stored choice goes.
    if get(keys::AVATAR_STYLE).is_some() {
        writes.push((keys::AVATAR_STYLE.to_owned(), None));
    }
    // Spec 075's pairing service is gone, under both the names it had. The
    // address behind them pointed at a WebSocket the wallet no longer opens,
    // so keeping it would only leave a stale answer for a question no screen
    // asks.
    for retired in [
        keys::RETIRED_CLEAR_SIGNER_TUNNEL,
        keys::RETIRED_CLEAR_SIGNER_RELAY,
    ] {
        if get(retired).is_some() {
            writes.push((retired.to_owned(), None));
        }
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
        let stored = entries(&[("vela.theme", "sepia"), ("vela.textScale", "huge")]);
        assert!(
            migrations(&stored).is_empty(),
            "a newer build's value is not overwritten"
        );
        let prefs = read(&stored);
        assert_eq!((prefs.theme, prefs.text_scale), ("system", "standard"));
        assert_eq!(text_scale_factor("xlarge"), 1.35);
        assert_eq!(
            read(&entries(&[("vela.language", "zh-TW")])).language,
            "zh-TW"
        );
    }

    #[test]
    fn both_spellings_of_the_pairing_service_are_removed() {
        // The channel went (2026-09-23); the addresses it pointed at go with
        // it, under the name they were stored by and the one before that.
        assert_eq!(
            migrations(&entries(&[
                ("vela.clearSignerTunnel", "wss://tunnel.example"),
                ("vela.theme", "dark"),
            ])),
            vec![("vela.clearSignerTunnel".to_owned(), None)]
        );
        assert_eq!(
            migrations(&entries(&[("vela.clearSignerRelay", "wss://old.example")])),
            vec![("vela.clearSignerRelay".to_owned(), None)]
        );
        // A device that somehow holds both loses both, newest first.
        assert_eq!(
            migrations(&entries(&[
                ("vela.clearSignerTunnel", "wss://new.example"),
                ("vela.clearSignerRelay", "wss://old.example"),
            ])),
            vec![
                ("vela.clearSignerTunnel".to_owned(), None),
                ("vela.clearSignerRelay".to_owned(), None),
            ]
        );
        // An empty value is still a key, and still goes.
        assert_eq!(
            migrations(&entries(&[("vela.clearSignerRelay", "")])),
            vec![("vela.clearSignerRelay".to_owned(), None)]
        );
        // Nothing stored under either key asks for no write at all.
        assert!(migrations(&entries(&[("vela.theme", "dark")])).is_empty());
    }

    #[test]
    fn debug_mode_reads_hidden_until_revealed() {
        let dev = |raw: &str| debug_mode(&entries(&[("vela.debugMode", raw)]), true);
        assert_eq!(debug_mode(&[], true), DebugMode::Hidden);
        assert_eq!(dev("off"), DebugMode::Off);
        assert_eq!(dev(" on "), DebugMode::On);
        // Anything this build does not write is hidden — and hidden is off.
        for raw in ["", "ON", "true", "1", "verbose"] {
            let mode = dev(raw);
            assert_eq!(mode, DebugMode::Hidden, "{raw:?}");
            assert!(!mode.is_on() && !mode.revealed());
        }
        // What is stored round-trips; hidden removes the key.
        for mode in [DebugMode::Off, DebugMode::On] {
            assert_eq!(DebugMode::read(mode.stored()), mode);
            assert_eq!(DebugMode::from_name(mode.name()), mode);
            assert!(mode.revealed());
        }
        assert_eq!(DebugMode::Hidden.stored(), None);
        assert_eq!(DebugMode::from_name("hidden"), DebugMode::Hidden);
        assert_eq!(DebugMode::switched(true), DebugMode::On);
        assert_eq!(DebugMode::switched(false), DebugMode::Off);
        assert!(DebugMode::On.is_on() && !DebugMode::Off.is_on());
        // Nothing to migrate: the key is left as it is.
        assert!(migrations(&entries(&[("vela.debugMode", "on")])).is_empty());
    }

    /// Owner, 2026-10-02: store builds forbid it entirely. Whatever a
    /// developer build left in the store, a store build reads hidden — off.
    #[test]
    fn a_store_build_has_no_debug_mode_whatever_is_stored() {
        for raw in ["on", "off", " on ", "anything"] {
            let mode = debug_mode(&entries(&[("vela.debugMode", raw)]), false);
            assert_eq!(mode, DebugMode::Hidden, "{raw:?}");
            assert!(!mode.is_on() && !mode.revealed());
        }
        assert_eq!(debug_mode(&[], false), DebugMode::Hidden);
        // The other preferences read the same in either build: debug mode
        // is not part of the shared record.
        let stored = entries(&[("vela.debugMode", "on"), ("vela.theme", "dark")]);
        assert_eq!(read(&stored), read(&entries(&[("vela.theme", "dark")])));
    }

    /// Seven taps in a run reveal the switch, once.
    #[test]
    fn seven_quick_taps_reveal_the_switch() {
        let mut taps = VersionTaps::default();
        for i in 0..VERSION_TAPS - 1 {
            let (next, revealed) = version_tapped(
                taps,
                1_000.0 + f64::from(i) * 300.0,
                DebugMode::Hidden,
                true,
            );
            assert!(!revealed, "tap {}", i + 1);
            assert_eq!(next.count, i + 1);
            taps = next;
        }
        let (after, revealed) =
            version_tapped(taps, 1_000.0 + 6.0 * 300.0, DebugMode::Hidden, true);
        assert!(revealed, "the seventh tap reveals");
        assert_eq!(after, VersionTaps::default(), "and the count starts over");
    }

    /// In a store build no number of taps reveals anything.
    #[test]
    fn a_store_build_never_reveals_the_switch() {
        let mut taps = VersionTaps::default();
        for i in 0..30 {
            let (next, revealed) =
                version_tapped(taps, f64::from(i) * 100.0, DebugMode::Hidden, false);
            assert!(!revealed, "tap {}", i + 1);
            assert_eq!(next, VersionTaps::default(), "nothing is even counted");
            taps = next;
        }
    }

    #[test]
    fn a_slow_tap_starts_the_count_again() {
        let mut taps = VersionTaps::default();
        for i in 0..6 {
            taps = version_tapped(taps, f64::from(i) * 500.0, DebugMode::Hidden, true).0;
        }
        assert_eq!(taps.count, 6);
        // Exactly the gap still counts …
        let (edge, revealed) =
            version_tapped(taps, 2_500.0 + VERSION_TAP_GAP_MS, DebugMode::Hidden, true);
        assert!(
            revealed,
            "a tap exactly {VERSION_TAP_GAP_MS} ms later is still in the run"
        );
        assert_eq!(edge, VersionTaps::default());
        // … one past it does not.
        let (late, revealed) = version_tapped(
            taps,
            2_500.0 + VERSION_TAP_GAP_MS + 1.0,
            DebugMode::Hidden,
            true,
        );
        assert!(!revealed);
        assert_eq!(late.count, 1, "a late tap is the first of a new run");
        // A clock that went backwards is no run either.
        let (back, revealed) = version_tapped(taps, 100.0, DebugMode::Hidden, true);
        assert!(!revealed);
        assert_eq!(back.count, 1);
    }

    #[test]
    fn once_revealed_nothing_counts() {
        for mode in [DebugMode::Off, DebugMode::On] {
            let mut taps = VersionTaps::default();
            for i in 0..20 {
                let (next, revealed) = version_tapped(taps, f64::from(i) * 100.0, mode, true);
                assert!(!revealed, "{mode:?} tap {i}");
                assert_eq!(next, VersionTaps::default());
                taps = next;
            }
        }
    }

    #[test]
    fn a_stored_avatar_style_is_removed_and_changes_nothing() {
        let stored = entries(&[("vela.avatarStyle", "initials"), ("vela.theme", "dark")]);
        assert_eq!(
            migrations(&stored),
            vec![("vela.avatarStyle".to_owned(), None)]
        );
        // Whatever it holds — even a value no build shipped — it goes.
        assert_eq!(
            migrations(&entries(&[("vela.avatarStyle", "photo")])),
            vec![("vela.avatarStyle".to_owned(), None)]
        );
        // It is not read: the preferences are what they would be without it.
        assert_eq!(read(&stored), read(&entries(&[("vela.theme", "dark")])));
        // Once removed, nothing more to do.
        assert!(migrations(&entries(&[("vela.theme", "dark")])).is_empty());
    }
}
