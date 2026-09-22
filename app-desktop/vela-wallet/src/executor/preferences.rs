//! Theme, language and text size — the person's display preferences, under
//! the keys every Vela shares (spec 072).
//!
//! The desktop drew them on Settings → Appearance and stored none of them:
//! the panel was a picture. (A fourth, the avatar style, is retired — spec
//! 074: every avatar is the identicon, and the core's migrations remove a
//! stored `vela.avatarStyle`.) The vocabulary, the defaults and how an older or
//! foreign spelling reads are the core's (`vela_core::prefs`); what is here is
//! only what a shell does — read the store at launch, rewrite the legacy
//! spellings once, remember the answer for the process, and write a choice
//! back under the shared key.
//!
//! The number, date and time formats live beside these in the same record
//! family (`vela.localePrefs`) but are applied by [`super::format_prefs`],
//! which owns the presets.

use std::sync::{Mutex, OnceLock, PoisonError};

use serde_json::Value;
use vela_core::prefs::{self, Prefs, keys};

use crate::executor::storage;

/// The preferences in force. Defaults until [`load`] reads the store — which
/// only the app's own start does, so a test never reads a developer's wallet.
fn cell() -> &'static Mutex<Prefs> {
    static PREFS: OnceLock<Mutex<Prefs>> = OnceLock::new();
    PREFS.get_or_init(|| Mutex::new(prefs::read(&[])))
}

/// At launch, before the first window: rewrite what an older shell wrote,
/// then read and apply what the store now says.
pub fn boot() {
    match migrate() {
        Ok(0) => {}
        Ok(count) => eprintln!("[vela-wallet] preferences: {count} legacy spelling(s) rewritten"),
        Err(error) => eprintln!("[vela-wallet] preferences could not be migrated: {error}"),
    }
    load();
}

/// Rewrite the known legacy spellings — the desktop's own `vela.formats`,
/// Android's `system` / `auto` / nested text size — and remove the retired
/// `vela.avatarStyle`, in one write. Returns how
/// many keys it touched; zero on a store that already agrees, which is why it
/// is safe at every launch.
pub fn migrate() -> Result<usize, storage::StorageError> {
    let writes = prefs::migrations(&storage::raw_entries()?);
    storage::apply_raw(&writes)?;
    Ok(writes.len())
}

/// Read the store and put what it says in force. Also what an erase calls to
/// return every preference to its default.
pub fn load() {
    let read = prefs::read(&storage::raw_entries().unwrap_or_default());
    apply(read);
}

fn apply(read: Prefs) {
    crate::theme::set_text_scale(prefs::text_scale_factor(read.text_scale));
    *cell().lock().unwrap_or_else(PoisonError::into_inner) = read;
}

/// The preferences in force.
#[must_use]
pub fn current() -> Prefs {
    cell()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// `system`, `light` or `dark`.
#[must_use]
pub fn theme() -> &'static str {
    cell().lock().unwrap_or_else(PoisonError::into_inner).theme
}

/// The language the person pinned, or `None` for "follow the system".
#[must_use]
pub fn pinned_language() -> Option<String> {
    let language = cell()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .language
        .clone();
    (language != prefs::AUTO_LANGUAGE).then_some(language)
}

/// A theme choice. A word the core does not ship is refused here rather than
/// stored: the store is read by four shells.
pub fn set_theme(theme: &str) {
    if prefs::THEMES.contains(&theme) {
        write(keys::THEME, theme);
    }
}

/// `auto`, or one of the locales this build ships.
pub fn set_language(language: &str) {
    if language == prefs::AUTO_LANGUAGE || vela_core::i18n::SUPPORTED.contains(&language) {
        write(keys::LANGUAGE, language);
    }
}

pub fn set_text_scale(level: &str) {
    if prefs::TEXT_SCALE_LEVELS
        .iter()
        .any(|(name, _)| *name == level)
    {
        write(keys::TEXT_SCALE, level);
    }
}

/// Store one preference and put it in force. Best effort, as every shell's
/// preference write is: the in-memory choice holds for this session either
/// way, and a failed write is said rather than swallowed.
fn write(key: &str, value: &str) {
    if let Err(error) = storage::write_value(key, Value::String(value.to_owned())) {
        eprintln!("[vela-wallet] {key} could not be saved: {error}");
    }
    let mut entries = storage::raw_entries().unwrap_or_default();
    // The write may have failed; the choice still holds for this session.
    entries.retain(|(stored, _)| stored != key);
    entries.push((key.to_owned(), value.to_owned()));
    apply(prefs::read(&entries));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage::tests::with_temp_state;
    use serde_json::json;

    fn seed(pairs: &[(&str, Value)]) {
        for (key, value) in pairs {
            if storage::write_value(key, value.clone()).is_err() {
                unreachable!("could not seed {key}");
            }
        }
    }

    /// The desktop's own old record moves to the shared one at launch, and the
    /// formats it held are the ones in force afterwards — T041.
    #[test]
    fn the_startup_migration_moves_the_desktops_formats_record() {
        with_temp_state("prefs-migrate-formats", || {
            seed(&[(
                keys::LEGACY_FORMATS,
                json!({ "number": "dot_comma", "date": "iso", "time": "h12" }),
            )]);
            assert!(migrate().unwrap_or_else(|error| unreachable!("{error}")) > 0);

            assert!(
                matches!(storage::read_value(keys::LEGACY_FORMATS), Ok(None)),
                "the old spelling is gone"
            );
            assert_eq!(
                storage::read_value(keys::LOCALE_PREFS).ok().flatten(),
                Some(json!({
                    "numberFormat": "dot_comma",
                    "dateFormat": "iso",
                    "timeFormat": "h12"
                })),
                "stored as the shared record — an object, not a quoted string"
            );
            let read = prefs::read(&storage::raw_entries().unwrap_or_default());
            assert_eq!(
                (read.number_format, read.date_format, read.time_format),
                ("dot_comma", "iso", "h12")
            );
            // Once is enough: a second launch has nothing to rewrite.
            assert_eq!(migrate().ok(), Some(0));
        });
    }

    /// A store another shell wrote — Android's words — reads as the shared
    /// ones after launch, and a newer build's value is left where it is.
    #[test]
    fn the_startup_migration_rewrites_other_shells_spellings_only() {
        with_temp_state("prefs-migrate-android", || {
            seed(&[
                (keys::THEME, json!("auto")),
                (keys::LANGUAGE, json!("system")),
                (
                    keys::LOCALE_PREFS,
                    json!({ "numberFormat": "indian", "textScale": 1 }),
                ),
            ]);
            if migrate().is_err() {
                unreachable!("migrate");
            }
            let stored = |key| storage::read_value(key).ok().flatten();
            assert_eq!(stored(keys::THEME), Some(json!("system")));
            assert_eq!(stored(keys::LANGUAGE), Some(json!("auto")));
            assert_eq!(stored(keys::TEXT_SCALE), Some(json!("small")));
            assert_eq!(
                stored(keys::LOCALE_PREFS)
                    .as_ref()
                    .and_then(|record| record.get("textScale"))
                    .cloned(),
                None,
                "the text size left the formats record"
            );
        });
        with_temp_state("prefs-migrate-newer", || {
            seed(&[(keys::TEXT_SCALE, json!("huge"))]);
            assert_eq!(migrate().ok(), Some(0));
            assert_eq!(
                storage::read_value(keys::TEXT_SCALE).ok().flatten(),
                Some(json!("huge")),
                "a value this build does not know is not overwritten"
            );
        });
    }

    /// A choice is stored under the shared key in the shared word, and a word
    /// the core does not ship is refused rather than written.
    #[test]
    fn a_choice_is_stored_under_the_shared_key() {
        with_temp_state("prefs-write", || {
            set_theme("dark");
            set_text_scale("large");
            // `auto` only: pinning a real language here would change the
            // strings every test running beside this one resolves.
            set_language("auto");
            set_theme("sepia");
            set_language("xx");
            let stored = |key| storage::read_value(key).ok().flatten();
            assert_eq!(stored(keys::THEME), Some(json!("dark")));
            assert_eq!(stored(keys::LANGUAGE), Some(json!("auto")));
            assert_eq!(stored(keys::TEXT_SCALE), Some(json!("large")));
            assert_eq!(theme(), "dark");
            assert_eq!(pinned_language(), None, "auto follows the system");
            // Leave the process as a default launch would find it.
            set_theme("system");
            set_text_scale("standard");
        });
    }

    /// Spec 074: every avatar is the identicon. A stored avatar style goes at
    /// launch, whatever it held, and nothing else it sat beside moves.
    #[test]
    fn the_startup_migration_removes_the_retired_avatar_style() {
        with_temp_state("prefs-migrate-avatar", || {
            seed(&[
                (keys::AVATAR_STYLE, json!("initials")),
                (keys::THEME, json!("dark")),
            ]);
            assert_eq!(migrate().ok(), Some(1));
            assert!(
                matches!(storage::read_value(keys::AVATAR_STYLE), Ok(None)),
                "the retired key is gone"
            );
            assert_eq!(
                storage::read_value(keys::THEME).ok().flatten(),
                Some(json!("dark"))
            );
            assert_eq!(migrate().ok(), Some(0));
        });
    }
}
