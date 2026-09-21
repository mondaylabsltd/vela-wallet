//! Settings' shared rules over UniFFI (spec 072): the preferences' record
//! format and how older spellings read, and the storage page's catalog and
//! erase rule — so no shell keeps its own copy to drift.

use std::collections::HashMap;

use vela_core::{prefs, storage_catalog};

fn entries_of(entries: HashMap<String, String>) -> Vec<(String, String)> {
    entries.into_iter().collect()
}

/// The five display preferences, read from the store whatever its spelling.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PrefsRecord {
    /// `system` | `light` | `dark`.
    pub theme: String,
    /// `auto`, or a locale tag.
    pub language: String,
    /// `initials` | `identicon`.
    pub avatar_style: String,
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
        avatar_style: read.avatar_style.to_owned(),
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
