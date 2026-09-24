//! Text size — the Appearance row that was drawn and never wired.
//!
//! `text_scale` and `segmented` took no click handler at all ("spec 023 is UI
//! only", in the component's own doc), so the size thumb sat on a stop that
//! meant nothing. A settings screen that shows a control which does nothing is
//! worse than one that shows no control.
//!
//! The avatar style that used to live here went with spec 074: the owner
//! dropped the choice between initials and the identicon, every avatar is the
//! identicon, and the core's launch migration removes a stored
//! `vela.avatarStyle`. A store key for a preference the app no longer has is
//! the same lie as an unwired control.
//!
//! The levels, the words and the store key are the web's — `vela.textScale`,
//! holding the same bare string — so the two clients describe one preference
//! rather than two that happen to look alike. Six
//! levels, not the mock's seven stops: the mock was a drawing, and the product
//! has six.

use std::sync::{Mutex, OnceLock, PoisonError};

use serde_json::Value;

use crate::executor::storage;

/// The store key, verbatim from `app-web/vela-wallet/src/lib/services/preferences.svelte.ts`.
pub const KEY_TEXT_SCALE: &str = "vela.textScale";

/// How much bigger or smaller every piece of type is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextScale {
    Compact,
    Small,
    #[default]
    Standard,
    Comfortable,
    Large,
    XLarge,
}

/// In the order the row draws them, smallest first.
pub const TEXT_SCALES: [TextScale; 6] = [
    TextScale::Compact,
    TextScale::Small,
    TextScale::Standard,
    TextScale::Comfortable,
    TextScale::Large,
    TextScale::XLarge,
];

impl TextScale {
    /// The multiplier, matching the web's `TEXT_SCALE_LEVELS` exactly. A
    /// person who set "large" on one client and opens the other should not
    /// find a different large.
    #[must_use]
    pub fn factor(self) -> f32 {
        match self {
            Self::Compact => 0.82,
            Self::Small => 0.91,
            Self::Standard => 1.0,
            Self::Comfortable => 1.1,
            Self::Large => 1.22,
            Self::XLarge => 1.35,
        }
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Small => "small",
            Self::Standard => "standard",
            Self::Comfortable => "comfortable",
            Self::Large => "large",
            Self::XLarge => "xlarge",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        TEXT_SCALES.into_iter().find(|scale| scale.word() == word)
    }

    /// Which stop the thumb sits on.
    #[must_use]
    pub fn index(self) -> usize {
        TEXT_SCALES
            .iter()
            .position(|scale| *scale == self)
            .unwrap_or(2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Prefs {
    text: TextScale,
}

static PREFS: OnceLock<Mutex<Prefs>> = OnceLock::new();

fn cell() -> &'static Mutex<Prefs> {
    PREFS.get_or_init(|| Mutex::new(load()))
}

fn read(prefs: impl FnOnce(&Prefs)) {
    let guard = cell().lock().unwrap_or_else(PoisonError::into_inner);
    prefs(&guard);
}

#[must_use]
pub fn text_scale() -> TextScale {
    let mut out = TextScale::default();
    read(|prefs| out = prefs.text);
    out
}

/// The multiplier every `theme::text_*` size is drawn through.
#[must_use]
pub fn text_factor() -> f32 {
    text_scale().factor()
}

pub fn set_text_scale(scale: TextScale) {
    update(|prefs| prefs.text = scale, KEY_TEXT_SCALE, scale.word());
}

fn update(apply: impl FnOnce(&mut Prefs), key: &str, word: &str) {
    {
        let mut guard = cell().lock().unwrap_or_else(PoisonError::into_inner);
        apply(&mut guard);
    }
    // Best effort, as every client's preference write is: the in-memory value
    // stays authoritative for this session either way.
    let _ = storage::write_value(key, Value::String(word.to_owned()));
}

fn load() -> Prefs {
    let word = |key: &str| {
        storage::read_value(key)
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::to_owned))
    };
    Prefs {
        text: word(KEY_TEXT_SCALE)
            .and_then(|w| TextScale::from_word(&w))
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words are a storage contract shared with the web client, and the
    /// factors are what "large" means. Both are pinned here because a silent
    /// change to either would make one client disagree with the other about a
    /// setting the person made once.
    #[test]
    fn the_words_and_factors_are_the_webs() {
        let words: Vec<&str> = TEXT_SCALES.iter().map(|s| s.word()).collect();
        assert_eq!(
            words,
            [
                "compact",
                "small",
                "standard",
                "comfortable",
                "large",
                "xlarge"
            ]
        );
        let factors: Vec<f32> = TEXT_SCALES.iter().map(|s| s.factor()).collect();
        assert_eq!(factors, [0.82, 0.91, 1.0, 1.1, 1.22, 1.35]);
        assert_eq!(
            TextScale::Standard.factor(),
            1.0,
            "standard changes nothing"
        );
        assert_eq!(TextScale::Standard.index(), 2);
    }

    /// An unreadable or absent value is the default, never a panic and never a
    /// silent half-setting.
    #[test]
    fn an_unknown_word_falls_back_to_the_default() {
        assert_eq!(TextScale::from_word("enormous"), None);
        assert_eq!(TextScale::from_word("large"), Some(TextScale::Large));
    }
}
