//! Settings' shared rules for the web (spec 072): the preferences' record
//! format and the storage page's catalog and erase rule — the phones'
//! `prefs_*` / `storage_*`, JSON in and out.

use serde_json::{json, Value};
use vela_core::{prefs, storage_catalog};
use wasm_bindgen::prelude::*;

fn entries_of(entries_json: &str) -> Vec<(String, String)> {
    serde_json::from_str::<serde_json::Map<String, Value>>(entries_json)
        .map(|map| {
            map.into_iter()
                .filter_map(|(key, value)| match value {
                    Value::String(text) => Some((key, text)),
                    Value::Null => None,
                    other => Some((key, other.to_string())),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `{key: rawValue}` → `{theme, language, avatarStyle, textScale,
/// textScaleFactor, numberFormat, dateFormat, timeFormat}`.
#[wasm_bindgen(js_name = prefsRead)]
pub fn prefs_read(entries_json: &str) -> String {
    let read = prefs::read(&entries_of(entries_json));
    json!({
        "theme": read.theme,
        "language": read.language,
        "avatarStyle": read.avatar_style,
        "textScale": read.text_scale,
        "textScaleFactor": prefs::text_scale_factor(read.text_scale),
        "numberFormat": read.number_format,
        "dateFormat": read.date_format,
        "timeFormat": read.time_format,
    })
    .to_string()
}

/// `{key: rawValue}` → `[{key, value | null}]`, the writes that bring an
/// older spelling to the shared record.
#[wasm_bindgen(js_name = prefsMigrations)]
pub fn prefs_migrations(entries_json: &str) -> String {
    let writes: Vec<Value> = prefs::migrations(&entries_of(entries_json))
        .into_iter()
        .map(|(key, value)| json!({ "key": key, "value": value }))
        .collect();
    Value::Array(writes).to_string()
}

/// The row a key belongs to, or `undefined`.
#[wasm_bindgen(js_name = storageItemOfKey)]
pub fn storage_item_of_key(key: &str) -> Option<String> {
    storage_catalog::item_of_key(key).map(|item| item.id.to_owned())
}

/// `[{id, group}]`, in the order the page draws them.
#[wasm_bindgen(js_name = storageItems)]
pub fn storage_items() -> String {
    let items: Vec<Value> = storage_catalog::ITEMS
        .iter()
        .map(|item| json!({ "id": item.id, "group": item.group.id() }))
        .collect();
    Value::Array(items).to_string()
}

#[wasm_bindgen(js_name = storageIsCacheKey)]
pub fn storage_is_cache_key(key: &str) -> bool {
    storage_catalog::is_cache_key(key)
}

#[wasm_bindgen(js_name = storageIsErasableKey)]
pub fn storage_is_erasable_key(key: &str) -> bool {
    storage_catalog::is_erasable_key(key)
}
