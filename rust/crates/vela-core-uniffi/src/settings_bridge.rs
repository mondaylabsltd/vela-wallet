//! Settings' shared rules over UniFFI (spec 072): the preferences' record
//! format and how older spellings read, and the storage page's catalog and
//! erase rule — so no shell keeps its own copy to drift.

use std::collections::HashMap;

use vela_core::{prefs, storage_catalog};

fn entries_of(entries: HashMap<String, String>) -> Vec<(String, String)> {
    entries.into_iter().collect()
}

/// The four display preferences, read from the store whatever its spelling.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PrefsRecord {
    /// `system` | `light` | `dark`.
    pub theme: String,
    /// `auto`, or a locale tag.
    pub language: String,
    /// `compact` … `xlarge`.
    pub text_scale: String,
    pub text_scale_factor: f64,
    pub number_format: String,
    pub date_format: String,
    pub time_format: String,
}

/// One store write: `value: None` removes the key.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct KeyWrite {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct TextScaleLevel {
    pub name: String,
    pub factor: f64,
}

/// Read the preferences from the store's `vela.*` entries.
#[uniffi::export]
pub fn prefs_read(entries: HashMap<String, String>) -> PrefsRecord {
    let read = prefs::read(&entries_of(entries));
    PrefsRecord {
        theme: read.theme.to_owned(),
        language: read.language,
        text_scale: read.text_scale.to_owned(),
        text_scale_factor: prefs::text_scale_factor(read.text_scale),
        number_format: read.number_format.to_owned(),
        date_format: read.date_format.to_owned(),
        time_format: read.time_format.to_owned(),
    }
}

/// The writes that bring an older shell's spellings to the shared record.
/// Empty when the store already agrees — safe at every launch.
#[uniffi::export]
pub fn prefs_migrations(entries: HashMap<String, String>) -> Vec<KeyWrite> {
    prefs::migrations(&entries_of(entries))
        .into_iter()
        .map(|(key, value)| KeyWrite { key, value })
        .collect()
}

/// The text-size slider's stops, in order.
#[uniffi::export]
pub fn prefs_text_scale_levels() -> Vec<TextScaleLevel> {
    prefs::TEXT_SCALE_LEVELS
        .iter()
        .map(|(name, factor)| TextScaleLevel {
            name: (*name).to_owned(),
            factor: *factor,
        })
        .collect()
}

/// Settings' debug mode in force on this build (spec 091): `hidden` (no
/// switch — and debug mode is off), `off` or `on`. `developer_build` is the
/// shell's build fact — Android `BuildConfig.DEBUG`, iOS `#if DEBUG`, the
/// desktop's `dev-fixtures` — and outside one the answer is always `hidden`,
/// whatever is stored (owner, 2026-10-02: store builds forbid it).
#[uniffi::export]
pub fn prefs_debug_mode(entries: HashMap<String, String>, developer_build: bool) -> String {
    prefs::debug_mode(&entries_of(entries), developer_build)
        .name()
        .to_owned()
}

/// What to store under `vela.debugMode` for the revealed switch set `on`
/// or off (spec 091).
#[uniffi::export]
pub fn prefs_debug_mode_value(on: bool) -> String {
    prefs::DebugMode::switched(on)
        .stored()
        .unwrap_or_default()
        .to_owned()
}

/// The count of taps on About's version so far (spec 091) — kept by the
/// shell while About is open, handed back with every tap.
#[derive(Debug, Clone, Copy, Default, PartialEq, uniffi::Record)]
pub struct VersionTaps {
    pub count: u32,
    pub last_ms: f64,
}

/// One tap's answer: the count to keep, and whether this tap revealed the
/// debug-mode switch (store `prefs_debug_mode_value(false)` and say so once).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct VersionTapAnswer {
    pub taps: VersionTaps,
    pub revealed: bool,
}

/// One tap on the version in About at `now_ms`, with the switch as it stands
/// (`hidden` | `off` | `on`). The core's rule: seven taps, each within a
/// second of the one before — and never outside a `developer_build`
/// (spec 091).
#[uniffi::export]
pub fn prefs_version_tapped(
    taps: VersionTaps,
    now_ms: f64,
    debug_mode: String,
    developer_build: bool,
) -> VersionTapAnswer {
    let (next, revealed) = prefs::version_tapped(
        prefs::VersionTaps {
            count: taps.count,
            last_ms: taps.last_ms,
        },
        now_ms,
        prefs::DebugMode::from_name(&debug_mode),
        developer_build,
    );
    VersionTapAnswer {
        taps: VersionTaps {
            count: next.count,
            last_ms: next.last_ms,
        },
        revealed,
    }
}

/// The `vela.localePrefs` record for three formats.
#[uniffi::export]
pub fn prefs_locale_json(
    number_format: String,
    date_format: String,
    time_format: String,
) -> String {
    prefs::locale_prefs_json(&number_format, &date_format, &time_format)
}

/// One drawn row of the storage page.
#[derive(Debug, Clone, uniffi::Record)]
pub struct StorageItemRecord {
    pub id: String,
    /// `user` | `cache` | `sessions`.
    pub group: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BytesDisplay {
    pub value: f64,
    pub unit: String,
}

/// The rows, in the order the page draws them.
#[uniffi::export]
pub fn storage_items() -> Vec<StorageItemRecord> {
    storage_catalog::ITEMS
        .iter()
        .map(|item| StorageItemRecord {
            id: item.id.to_owned(),
            group: item.group.id().to_owned(),
        })
        .collect()
}

/// The row a key belongs to; `None` for the accounts and preferences.
#[uniffi::export]
pub fn storage_item_of_key(key: String) -> Option<String> {
    storage_catalog::item_of_key(&key).map(|item| item.id.to_owned())
}

/// Would "clear all caches" remove this key?
#[uniffi::export]
pub fn storage_is_cache_key(key: String) -> bool {
    storage_catalog::is_cache_key(&key)
}

/// Is this key the wallet's at all (counted in the storage total)?
#[uniffi::export]
pub fn storage_is_ours(key: String) -> bool {
    storage_catalog::is_ours(&key)
}

/// Would "erase this device" delete this key? Scan the namespace, keep the
/// keep-list.
#[uniffi::export]
pub fn storage_is_erasable_key(key: String) -> bool {
    storage_catalog::is_erasable_key(&key)
}

/// Records in a stored list value; `None` when it is not a list.
#[uniffi::export]
pub fn storage_records_in(value: String) -> Option<u32> {
    storage_catalog::records_in(&value).map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

/// A byte count in 1024s, for the shell to format in the person's numbers.
#[uniffi::export]
pub fn storage_bytes_display(bytes: u64) -> BytesDisplay {
    let (value, unit) = storage_catalog::bytes_display(bytes);
    BytesDisplay {
        value,
        unit: unit.to_owned(),
    }
}
