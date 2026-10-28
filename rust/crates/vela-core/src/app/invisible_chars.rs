//! Characters a person cannot see, or that reorder what they see — and the
//! one rule every surface uses to show them anyway (clear-signing audit
//! cs-11).
//!
//! "Approve transfer to Bob <U+202E>gpj.exe<U+202C> ok" — a RIGHT-TO-LEFT
//! OVERRIDE in the middle — is drawn by every browser as "Approve transfer to
//! Bob exe.jpg ok": the override turns the rest of the run around, and the
//! bytes signed are not the words read. Zero-width characters split or join
//! words with nothing on screen to say so. A signing surface must never draw such a character as itself:
//! [`display_safe`] replaces each with a visible marker (`⟨U+202E⟩`) for the
//! text that is DRAWN, and [`has_invisible`] says the request carries one, so
//! the sheet can say so in words. What is signed is never touched.
//!
//! The classification ([`is_invisible`]) is deliberately wide: the bidi
//! controls, the zero-width characters, every other format character
//! (Unicode `Cf`), the line and paragraph separators, the C0 and C1 controls
//! other than tab, line feed and carriage return, and the fillers that draw
//! as blank glyphs. Two characters are spared where they do their ordinary
//! work: ZERO WIDTH JOINER and NON-JOINER between two emoji or two characters
//! of a script beyond ASCII — the family emoji, Persian and Indic words.
//! Between anything else (inside "Bob", at an edge) they are hidden text.
//!
//! Reused by every surface that draws untrusted text for a signature: the
//! clear-signing message and typed-data projections today, an agent's
//! description of a request tomorrow.

/// How a hidden character is drawn: `⟨U+202E⟩`. The brackets are U+27E8 and
/// U+27E9, which no shell's text renderer reorders or hides.
#[must_use]
pub fn marker(c: char) -> String {
    format!("\u{27E8}U+{:04X}\u{27E9}", c as u32)
}

/// Is `c` a character that draws as nothing, or that changes the direction or
/// order of what is drawn around it? Context-free; [`has_invisible`] and
/// [`display_safe`] add the joiner exemption (module doc).
#[must_use]
pub fn is_invisible(c: char) -> bool {
    let code = c as u32;
    matches!(
        code,
        // C0 controls other than tab, line feed and carriage return; DEL; C1.
        0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F..=0x9F
        // SOFT HYPHEN, COMBINING GRAPHEME JOINER, ARABIC LETTER MARK.
        | 0x00AD | 0x034F | 0x061C
        // Hangul fillers: letters that draw as blank space.
        | 0x115F | 0x1160 | 0x3164 | 0xFFA0
        // Arabic, Syriac and Kaithi number/abbreviation signs (format chars).
        | 0x0600..=0x0605 | 0x06DD | 0x070F | 0x0890 | 0x0891 | 0x08E2
        | 0x110BD | 0x110CD
        // MONGOLIAN VOWEL SEPARATOR.
        | 0x180E
        // Zero width space/non-joiner/joiner, LRM, RLM.
        | 0x200B..=0x200F
        // Line and paragraph separators; LRE, RLE, PDF, LRO, RLO.
        | 0x2028..=0x202E
        // Word joiner, invisible operators, LRI, RLI, FSI, PDI, deprecated
        // format characters.
        | 0x2060..=0x2064 | 0x2066..=0x206F
        // ZERO WIDTH NO-BREAK SPACE (the BOM), interlinear annotation.
        | 0xFEFF | 0xFFF9..=0xFFFB
        // Egyptian hieroglyph and shorthand format controls, musical
        // formatting.
        | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
        // Tags (they spell text no one sees).
        | 0xE0001 | 0xE0020..=0xE007F
    )
}

/// A joiner (ZWNJ, ZWJ) that does its ordinary work between `before` and
/// `after`: two emoji, or two visible characters outside ASCII — the letters
/// and signs of Persian, Devanagari and the other scripts that need them.
fn ordinary_joiner(c: char, before: Option<char>, after: Option<char>) -> bool {
    if !matches!(c, '\u{200C}' | '\u{200D}') {
        return false;
    }
    let (Some(before), Some(after)) = (before, after) else {
        return false;
    };
    let emoji_side = |ch: char| is_pictographic(ch) || ch == '\u{FE0F}';
    // A virama is a mark, not a letter, so "outside ASCII and drawn" rather
    // than `is_alphabetic`.
    let script_letter = |ch: char| !ch.is_ascii() && !ch.is_whitespace() && !is_invisible(ch);
    (emoji_side(before) && is_pictographic(after))
        || (script_letter(before) && script_letter(after))
}

/// The emoji blocks a ZWJ sequence is built from — close enough for the
/// exemption: a false match only spares a joiner between two pictographs.
fn is_pictographic(c: char) -> bool {
    matches!(
        c as u32,
        0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x1F000..=0x1FAFF
    )
}

/// Each character of `text` with whether it is hidden in its place.
fn classify(text: &str) -> impl Iterator<Item = (char, bool)> + '_ {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len()).map(move |i| {
        let c = chars[i];
        let before = i.checked_sub(1).map(|j| chars[j]);
        let after = chars.get(i + 1).copied();
        (c, is_invisible(c) && !ordinary_joiner(c, before, after))
    })
}

/// Does `text` carry a character [`display_safe`] would replace?
#[must_use]
pub fn has_invisible(text: &str) -> bool {
    // Fast path: nothing to look at.
    if !text.chars().any(is_invisible) {
        return false;
    }
    classify(text).any(|(_, hidden)| hidden)
}

/// One run of drawn text: as written, or a hidden character's [`marker`]
/// (`marker: true`), which a shell may draw muted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRun {
    pub text: String,
    pub marker: bool,
}

/// `text` as runs: the visible stretches as written, each hidden character as
/// its own marker run. Empty text is no runs.
#[must_use]
pub fn display_runs(text: &str) -> Vec<DisplayRun> {
    let mut runs: Vec<DisplayRun> = Vec::new();
    for (c, hidden) in classify(text) {
        if hidden {
            runs.push(DisplayRun {
                text: marker(c),
                marker: true,
            });
            continue;
        }
        match runs.last_mut() {
            Some(last) if !last.marker => last.text.push(c),
            _ => runs.push(DisplayRun {
                text: c.to_string(),
                marker: false,
            }),
        }
    }
    runs
}

/// The text to DRAW for `text`: every hidden or direction-changing character
/// replaced by its visible marker, everything else as written. Never use it
/// for what is signed.
#[must_use]
pub fn display_safe(text: &str) -> String {
    if !has_invisible(text) {
        return text.to_owned();
    }
    display_runs(text).into_iter().map(|run| run.text).collect()
}

/// Does any string in a JSON value — a key or a string value, at any depth —
/// carry a hidden character? For a payload the sheet draws only part of.
#[must_use]
pub fn json_has_invisible(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => has_invisible(s),
        serde_json::Value::Array(items) => items.iter().any(json_has_invisible),
        serde_json::Value::Object(map) => map
            .iter()
            .any(|(key, value)| has_invisible(key) || json_has_invisible(value)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidi_controls_are_hidden_and_shown_as_markers() {
        let text = "Approve transfer to Bob \u{202E}gpj.exe\u{202C}\u{200B}\u{200B} ok";
        assert!(has_invisible(text));
        assert_eq!(
            display_safe(text),
            "Approve transfer to Bob ⟨U+202E⟩gpj.exe⟨U+202C⟩⟨U+200B⟩⟨U+200B⟩ ok"
        );
    }

    #[test]
    fn every_listed_control_is_hidden() {
        for c in [
            '\u{202A}',
            '\u{202B}',
            '\u{202C}',
            '\u{202D}',
            '\u{202E}',
            '\u{2066}',
            '\u{2067}',
            '\u{2068}',
            '\u{2069}',
            '\u{200E}',
            '\u{200F}',
            '\u{061C}',
            '\u{200B}',
            '\u{200C}',
            '\u{200D}',
            '\u{2060}',
            '\u{FEFF}',
            '\u{00AD}',
            '\u{2028}',
            '\u{2029}',
            '\u{0000}',
            '\u{001B}',
            '\u{007F}',
            '\u{0085}',
            '\u{E0041}',
            '\u{3164}',
        ] {
            assert!(is_invisible(c), "U+{:04X} should be hidden", c as u32);
            assert!(
                has_invisible(&format!("a{c}b")),
                "U+{:04X} in a word",
                c as u32
            );
        }
    }

    #[test]
    fn ordinary_text_is_untouched() {
        for text in [
            "Welcome to Explore! Click to sign in.",
            "line one\nline two\r\n\ttabbed",
            "中文签名 · 日本語 · 한국어 · Ελληνικά · עברית · العربية",
            "emoji 🚀✅",
            "",
        ] {
            assert!(!has_invisible(text), "{text:?}");
            assert_eq!(display_safe(text), text);
        }
    }

    #[test]
    fn joiners_doing_their_work_are_spared() {
        // The family emoji: man ZWJ woman ZWJ girl.
        assert!(!has_invisible("👨\u{200D}👩\u{200D}👧"));
        // Heart with the emoji presentation selector, joined to fire.
        assert!(!has_invisible("❤\u{FE0F}\u{200D}🔥"));
        // Persian: a ZWNJ between two Arabic-script letters ("mi-khaham").
        assert!(!has_invisible("می\u{200C}خواهم"));
        // Devanagari half-form with ZWJ.
        assert!(!has_invisible("क्\u{200D}ष"));
    }

    #[test]
    fn joiners_inside_latin_words_or_at_edges_are_hidden() {
        assert!(has_invisible("B\u{200D}ob"));
        assert!(has_invisible("Bob\u{200C}"));
        assert!(has_invisible("\u{200D}👩"));
        assert_eq!(display_safe("B\u{200C}ob"), "B⟨U+200C⟩ob");
    }

    #[test]
    fn runs_split_markers_from_text() {
        assert_eq!(
            display_runs("a\u{202E}bc"),
            vec![
                DisplayRun {
                    text: "a".into(),
                    marker: false
                },
                DisplayRun {
                    text: "⟨U+202E⟩".into(),
                    marker: true
                },
                DisplayRun {
                    text: "bc".into(),
                    marker: false
                },
            ]
        );
        assert!(display_runs("").is_empty());
    }

    #[test]
    fn markers_name_astral_code_points_in_full() {
        assert_eq!(marker('\u{E0041}'), "⟨U+E0041⟩");
        assert_eq!(marker('\u{200B}'), "⟨U+200B⟩");
    }

    #[test]
    fn json_is_searched_at_every_depth_and_in_keys() {
        let deep: serde_json::Value = serde_json::json!({ "a": [1, { "b": "x\u{202E}y" }] });
        assert!(json_has_invisible(&deep));
        let key: serde_json::Value = serde_json::json!({ "to\u{200B}": "Bob" });
        assert!(json_has_invisible(&key));
        let clean: serde_json::Value = serde_json::json!({ "to": "Bob", "n": [1, true, null] });
        assert!(!json_has_invisible(&clean));
    }
}
