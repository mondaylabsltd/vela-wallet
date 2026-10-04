//! What a person typed into an amount field, as the text the core can read
//! (issue 231; moved here from the web's `amount-text.ts` by spec 073).
//!
//! The core reads ONE shape: ASCII digits and a `.`. In fiat mode the send
//! machine parses the figure the way `parseFloat` does — the longest numeric
//! prefix — so "4,5" from a decimal-comma keypad is read as 4 and a different
//! sum is sent, silently. In token mode the same text enables Continue and is
//! then refused when the call is built. Neither is something a person can work
//! out from the screen, so every shell cleans the text HERE, in its field's
//! change handler, before it is dispatched, and writes the result back into
//! the field so what is on screen is what was sent on.
//!
//! It runs in the field, not in the machine, on purpose: a shell whose view
//! lags the typing (Android, device-found) would otherwise send "4,5" after
//! the core had already read "4," as "4.", and the machine — comparing with
//! "4." — could not tell a decimal mark from grouping.
//!
//! The field's own text is always dot-decimal: the core echoes what it
//! stored, and Max and the ⇄ swap write "0.00075" whatever the number preset.
//! So a `.` is never treated as grouping while typing — dropping it would turn
//! the "0.5" the wallet itself put there into "05" on the next keystroke. What
//! the preset decides is what a `,` means:
//!
//! - decimal-comma preset, no `.` yet → the `,` IS the decimal mark ("4,5" → "4.5");
//! - a single typed `,`, no `.` yet → the same, under any preset (below);
//! - otherwise → a `,` is grouping and is dropped ("1,234.5" → "1234.5") —
//!   but under a decimal-point preset a figure with no `.` whose commas are
//!   not grouping ("1,5", "1,50") has no safe reading when it arrives whole,
//!   and is refused like any paste with no reading.
//!
//! A `,` is never blindly mapped to `.`: a pasted "1,234.56" would become
//! "1.234.56", which reads as 1.234. A PASTED figure that is unmistakably
//! grouped — a decimal part after one kind of mark, thousands after the other
//! — is read for what it is under any preset. Only a paste gets that reading:
//! a stray `,` typed into the middle of "1.23456" can spell "1.234,56" too,
//! and one keystroke must not move a figure a thousandfold.
//!
//! A `,` typed as ONE keystroke into a figure with no `.` yet is a decimal
//! mark under any preset (`previous` is how a keystroke is told from anything
//! else). The number preset follows the app's setting and a phone's decimal
//! pad follows the device's region; where the two disagree the pad's only
//! separator is `,`, and dropping it left a person unable to type a fraction
//! at all — "4,5" became 45 with no sign a key had been refused. Nobody groups
//! thousands by hand one key at a time; a pasted or autofilled "1,234" arrives
//! whole, is not a keystroke, and is still grouping.
//!
//! A PASTE that cannot be read as one figure is refused whole (`None`), and
//! the field keeps what it had: "1.5e-7" — how many screens print a tiny
//! balance — salvaged digit by digit is 1.57, ten million times the figure
//! copied; so is "0x10", and "4.5.6" has no reading at all. While TYPING the
//! same stray character is simply dropped, because there the figure on screen
//! does not change and the person sees their key do nothing.
//!
//! A figure has ONE spelling (issue #421). "08" was kept as typed, shown as
//! "08 POL", read as 8 — and a person who meant 0.8 and missed the point
//! sent ten times what they meant, with Continue lit and nothing on screen
//! saying which reading was taken. So a zero that leads a whole part with a
//! digit after it is not kept: "0" then "8" is "8" (the key replaces the
//! zero), "00" is "0", a pasted "008.5" is "8.5". Only a decimal mark may
//! follow a leading zero — "0.08", "0.0" and "0." stay as typed. A figure that
//! starts at its decimal mark is given the zero it reads with: "." is "0.",
//! ".5" is "0.5". The rule runs on the cleaned dot-decimal text, so it is the
//! same under every preset: a decimal-comma person's "0,8" is "0.8", which
//! their field shows as "0,8". And because the field shows what this returns
//! and sends THAT on, the figure on screen, the fiat line under it and the
//! amount Continue judges are one number.
//!
//! The same reading keeps a pasted "0,800" or "0.800.000" from passing as
//! thousands grouping: no grouped figure starts with a zero, so under a
//! preset whose decimal mark is the other one it has no safe reading and is
//! refused — 800 is a thousand times somebody's 0,800.
//!
//! Strings only. This is an amount of money; it never passes through a number.

use super::number::NumberPreset;

/// How the edit reached the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// The shell knows it was typed (the web's `inputType`).
    Typed,
    /// The shell knows it was pasted or dropped.
    Pasted,
    /// The shell cannot tell (a native text field). Anything but a single
    /// added character is read as a paste: refusing a figure that has no
    /// reading is always safer than salvaging a different one, and on clean
    /// text the two readings agree.
    Unknown,
}

impl Entry {
    /// The bindings' spelling: `Some(pasted)` when the shell knows, `None`
    /// when it cannot tell.
    #[must_use]
    pub const fn from_pasted(pasted: Option<bool>) -> Self {
        match pasted {
            Some(true) => Entry::Pasted,
            Some(false) => Entry::Typed,
            None => Entry::Unknown,
        }
    }
}

/// The cleaned figure, or `None` for a paste with no reading as ONE figure
/// (the field then keeps `previous`).
///
/// `previous` is the field's text before this edit. Without it an edit is
/// never a keystroke.
#[must_use]
pub fn clean(
    raw: &str,
    preset: NumberPreset,
    entry: Entry,
    previous: Option<&str>,
) -> Option<String> {
    let pasted = match entry {
        Entry::Typed => false,
        Entry::Pasted => true,
        Entry::Unknown => {
            previous.is_none_or(|before| raw.chars().count() > before.chars().count() + 1)
        }
    };
    let preset_comma = preset.separators().decimal == ",";

    // A letter BETWEEN digits is an exponent or a hex figure, not a label
    // beside a number ("$4.00", "4 USDT" are fine).
    if pasted && letter_between_digits(raw) {
        return None;
    }

    // Spaces and apostrophes are grouping in some presets; letters, signs and
    // exponents are nothing an amount is written with.
    let text: String = raw
        .chars()
        .map(plain)
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();

    // "1,234.56" / "1,234,567" — and their mirror, "1.234,56" / "1.234.567".
    if pasted {
        if grouped(&text, ',', '.') {
            return Some(text.replace(',', ""));
        }
        if grouped(&text, '.', ',') {
            return Some(text.replace('.', "").replacen(',', ".", 1));
        }
    }

    let keystroke = previous.is_some_and(|before| {
        text.chars().count() == before.chars().count() + 1 && text.replacen(',', "", 1) == before
    });
    let comma_is_decimal = (preset_comma || keystroke) && !text.contains('.');
    // Under a decimal-point preset, a comma that arrived with other text (a
    // paste, an autofill — not the one key) in a figure with no point is
    // grouping only if it IS grouping ("1,234", "12,34,567"). "1,5" or
    // "1,50" is somebody's decimal comma as likely as a slip, and neither
    // reading is safe: dropped as grouping it is ten times the figure.
    if !comma_is_decimal
        && !keystroke
        && text.contains(',')
        && !text.contains('.')
        && !grouping_only(&text)
    {
        return None;
    }
    // Two decimal marks in a paste: there is no figure here to read.
    if pasted && comma_is_decimal && text.matches(',').count() > 1 {
        return None;
    }
    let dotted = if comma_is_decimal {
        text.replacen(',', ".", 1)
    } else {
        text
    };
    // One decimal mark, the first; every later mark and every comma still
    // standing goes.
    let without_commas = dotted.replace(',', "");
    let mut parts = without_commas.split('.');
    let whole = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    if pasted && rest.len() > 1 {
        return None;
    }
    Some(if rest.is_empty() {
        one_spelling(whole, false).to_owned()
    } else {
        format!("{}.{}", one_spelling(whole, true), rest.concat())
    })
}

/// A whole part as it is written once (issue #421): no zero leading another
/// digit ("08" → "8", "00" → "0"), and a "0" before a decimal mark that has
/// nothing in front of it ("." → "0."). `pointed` = a decimal mark follows.
fn one_spelling(whole: &str, pointed: bool) -> &str {
    let trimmed = whole.trim_start_matches('0');
    if trimmed.is_empty() && (pointed || !whole.is_empty()) {
        "0"
    } else {
        trimmed
    }
}

/// Where the caret belongs in `clean`, having been at `caret` in `raw`: after
/// as many KEPT characters as stood before it. Cleaning the text before the
/// caret on its own gave a different answer than cleaning the whole — in
/// "1,2.5" the comma is dropped, but "1," alone reads it as a decimal mark —
/// and the caret landed a character late.
///
/// Positions are UTF-16 code units, which is what a browser's, Android's and
/// UIKit's text selection count in; `clean` is ASCII, so its units are its
/// characters.
#[must_use]
pub fn caret_after_clean(raw: &str, clean: &str, caret: usize) -> usize {
    let clean: Vec<char> = clean.chars().collect();
    let mut kept = 0;
    let mut at = 0;
    for ch in raw.chars() {
        if at >= caret || kept >= clean.len() {
            break;
        }
        at += ch.len_utf16();
        let ch = plain(ch);
        let next = clean[kept];
        if ch == next || (is_mark(ch) && is_mark(next)) {
            kept += 1;
        } else if kept == 0
            && is_mark(ch)
            && next == '0'
            && clean.get(1).copied().is_some_and(is_mark)
        {
            // The "0" `clean` gave a figure that started at its mark
            // ("." → "0."): the caret goes after the mark, past the zero —
            // left between the two, the next digit would land in front of
            // the point and "0.5" would read 5.
            kept = 2;
        }
    }
    kept
}

/// The preset a shell names on the wire (`comma_dot`, `dot_comma`,
/// `space_comma`, `indian`). `auto` and anything unknown read as `comma_dot`:
/// the shells resolve `auto` before they call, and under a decimal-point
/// preset a typed `,` is still a decimal mark (the keystroke rule).
#[must_use]
pub fn preset_of(key: &str) -> NumberPreset {
    match key {
        "dot_comma" => NumberPreset::DotComma,
        "space_comma" => NumberPreset::SpaceComma,
        "indian" => NumberPreset::Indian,
        _ => NumberPreset::CommaDot,
    }
}

const fn is_mark(ch: char) -> bool {
    matches!(ch, '.' | ',')
}

/// One character, as the ASCII an amount is written with. Arabic-Indic and
/// Persian digits as `parse_locale_number` maps them, and the full-width forms
/// a ja / zh / ko keyboard produces — WITH their decimal marks: mapping the
/// digits and dropping the mark between them turned "٤٫٥" into 45.
fn plain(ch: char) -> char {
    let code = u32::from(ch);
    let digit = match code {
        0x0660..=0x0669 => Some(code - 0x0660),
        0x06F0..=0x06F9 => Some(code - 0x06F0),
        0xFF10..=0xFF19 => Some(code - 0xFF10),
        _ => None,
    };
    if let Some(d) = digit.and_then(|d| char::from_digit(d, 10)) {
        return d;
    }
    match ch {
        // "٫" is the Arabic decimal separator and nothing else; "．" and "。"
        // are what the `.` key gives in a full-width / Chinese-punctuation
        // keyboard.
        '٫' | '．' | '。' => '.',
        '，' => ',',
        other => other,
    }
}

/// `/\d[a-zA-Z]+[-+]?\d/` — an exponent ("1.5e-7") or a hex figure ("0x10").
fn letter_between_digits(raw: &str) -> bool {
    let chars: Vec<char> = raw.chars().collect();
    (0..chars.len()).any(|start| {
        if !chars[start].is_ascii_digit() {
            return false;
        }
        let mut i = start + 1;
        let letters = i;
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i == letters {
            return false;
        }
        if i < chars.len()
            && matches!(chars[i], '-' | '+')
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            return true;
        }
        chars.get(i).is_some_and(char::is_ascii_digit)
    })
}

/// `^\d{1,3}(?:,\d{3})+$` or the Indian `^\d{1,2}(?:,\d{2})*,\d{3}$` — a
/// whole number whose every comma is a thousands (or lakh) separator.
fn grouping_only(text: &str) -> bool {
    let groups: Vec<&str> = text.split(',').collect();
    let digits = |g: &str| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit());
    if groups.len() < 2 || !groups.iter().all(|g| digits(g)) {
        return false;
    }
    let (first, rest) = (groups[0], &groups[1..]);
    // No grouped figure starts with a zero: "0,800" is somebody's 0.8.
    if first.starts_with('0') {
        return false;
    }
    let western = first.len() <= 3 && rest.iter().all(|g| g.len() == 3);
    let (last, middle) = rest.split_last().unwrap_or((&"", &[]));
    let indian = first.len() <= 2 && last.len() == 3 && middle.iter().all(|g| g.len() == 2);
    western || indian
}

/// `^\d{1,3}(?:G\d{3})+D\d+$` or `^\d{1,3}(?:G\d{3}){2,}$` — a figure grouped
/// by `group` with an optional `decimal` part; without one, two groups at
/// least ("1,234" alone could be a decimal comma).
fn grouped(text: &str, group: char, decimal: char) -> bool {
    let bytes = text.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let lead = digits(0);
    // …and never with a zero ("0.800.000" is not eight hundred thousand).
    if !(1..=3).contains(&lead) || bytes[0] == b'0' {
        return false;
    }
    let mut i = lead;
    let mut groups = 0;
    // A group is exactly three digits: "1,2345" is not grouping.
    while i < bytes.len() && char::from(bytes[i]) == group && digits(i + 1) == 3 {
        groups += 1;
        i += 4;
    }
    if i == bytes.len() {
        return groups >= 2;
    }
    groups >= 1
        && char::from(bytes[i]) == decimal
        && digits(i + 1) >= 1
        && i + 1 + digits(i + 1) == bytes.len()
}

#[cfg(test)]
mod tests {
    //! The web's `amount-text.test.ts`, case for case: the cases where
    //! cleaning the text could itself move a figure — which is the only kind
    //! of bug worth having here.
    use super::*;

    const DOT: NumberPreset = NumberPreset::CommaDot;
    const COMMA: NumberPreset = NumberPreset::DotComma;

    fn typed(raw: &str, preset: NumberPreset) -> Option<String> {
        clean(raw, preset, Entry::Typed, None)
    }
    fn pasted(raw: &str, preset: NumberPreset) -> Option<String> {
        clean(raw, preset, Entry::Pasted, None)
    }
    fn s(v: &str) -> Option<String> {
        Some(v.to_owned())
    }

    #[test]
    fn a_decimal_comma_preset_reads_a_typed_comma_as_the_decimal_mark() {
        assert_eq!(typed("4,5", COMMA), s("4.5"));
        assert_eq!(typed("4,", COMMA), s("4."));
        // The zero a figure reads with (issue #421).
        assert_eq!(typed(",5", COMMA), s("0.5"));
        // The space-grouped preset is a decimal-comma preset too.
        assert_eq!(typed("4,5", NumberPreset::SpaceComma), s("4.5"));
    }

    #[test]
    fn the_fields_own_dot_decimal_text_survives_the_next_keystroke() {
        // The core echoes "4.5"; Max writes "0.00075". Dropping the dot as
        // "grouping" would make 4.56 into 456 one keystroke after it was right.
        assert_eq!(typed("4.56", COMMA), s("4.56"));
        assert_eq!(typed("0.000751", COMMA), s("0.000751"));
    }

    #[test]
    fn a_second_mark_is_dropped_rather_than_re_reading_the_figure() {
        assert_eq!(typed("4.5,", COMMA), s("4.5"));
        assert_eq!(typed("4,5,6", COMMA), s("4.56"));
        // Typed into the middle of 1.23456 — NOT one thousand two hundred.
        assert_eq!(typed("1.234,56", COMMA), s("1.23456"));
    }

    #[test]
    fn a_pasted_grouped_figure_is_read_for_what_it_is() {
        assert_eq!(pasted("1.234,56", COMMA), s("1234.56"));
        assert_eq!(pasted("1.234.567", COMMA), s("1234567"));
        assert_eq!(pasted("1 234,56", COMMA), s("1234.56"));
        // Written the other way round, it is still unmistakable.
        assert_eq!(pasted("1,234.56", COMMA), s("1234.56"));
    }

    #[test]
    fn a_decimal_point_preset_drops_commas_as_grouping() {
        // The blind `,` → `.` would make this 1.234.
        assert_eq!(typed("1,234.56", DOT), s("1234.56"));
        assert_eq!(typed("1,234.5", DOT), s("1234.5"));
        assert_eq!(typed("12,34,567.89", DOT), s("1234567.89"));
        assert_eq!(pasted("1,234,567", DOT), s("1234567"));
    }

    #[test]
    fn a_decimal_point_preset_keeps_the_first_point() {
        assert_eq!(typed("4.5.", DOT), s("4.5"));
        assert_eq!(typed("1.5.2", DOT), s("1.52"));
    }

    #[test]
    fn a_clean_figure_is_left_exactly_as_typed() {
        for text in [
            "",
            "0",
            "0.",
            "0.0",
            "0.08",
            "4",
            "4.",
            "0.50",
            "10",
            "100.05",
            "53.483600000000000001",
        ] {
            for preset in [DOT, COMMA] {
                assert_eq!(typed(text, preset), s(text), "{text}");
                // A fixed point under every reading, so a shell that cleaned
                // first and a core that cleans again agree.
                assert_eq!(pasted(text, preset), s(text), "{text}");
                assert_eq!(
                    clean(text, preset, Entry::Unknown, Some("")),
                    s(text),
                    "{text}"
                );
            }
        }
    }

    /// Issue #421: "08" was kept, shown as "08 POL" and read as 8 — ten times
    /// what a person who missed the point meant. A zero leading another digit
    /// is not kept; only a decimal mark may follow it.
    #[test]
    fn a_zero_leading_a_digit_is_not_kept() {
        for preset in [DOT, COMMA, NumberPreset::SpaceComma, NumberPreset::Indian] {
            // "0" on screen, then "8": the key replaces the zero.
            assert_eq!(clean("08", preset, Entry::Unknown, Some("0")), s("8"));
            assert_eq!(clean("08", preset, Entry::Typed, Some("0")), s("8"));
            // "0" then "0" stays one zero.
            assert_eq!(clean("00", preset, Entry::Unknown, Some("0")), s("0"));
            // A decimal mark may follow it, and digits that mark.
            assert_eq!(
                clean("0.08", preset, Entry::Unknown, Some("0.0")),
                s("0.08")
            );
            assert_eq!(clean("0.0", preset, Entry::Unknown, Some("0.")), s("0.0"));
            assert_eq!(clean("0.", preset, Entry::Unknown, Some("0")), s("0."));
            // A figure that starts at its mark reads with its zero.
            assert_eq!(clean(".", preset, Entry::Unknown, Some("")), s("0."));
            assert_eq!(clean(".5", preset, Entry::Unknown, Some("5")), s("0.5"));
            // Pasted: the zeros go, the figure stays.
            assert_eq!(pasted("008.5", preset), s("8.5"));
            assert_eq!(clean("008.5", preset, Entry::Unknown, Some("")), s("8.5"));
            assert_eq!(pasted("0008", preset), s("8"));
            assert_eq!(pasted("00.5", preset), s("0.5"));
            assert_eq!(pasted("000", preset), s("0"));
            // Zeros AFTER the first digit are the figure's.
            assert_eq!(pasted("800", preset), s("800"));
            assert_eq!(pasted("100.05", preset), s("100.05"));
            assert_eq!(pasted("0.050", preset), s("0.050"));
            // Deleting down to a zero-led figure: "10" without its 1 is "0".
            assert_eq!(clean("00", preset, Entry::Unknown, Some("100")), s("0"));
            assert_eq!(clean("05", preset, Entry::Unknown, Some("0.5")), s("5"));
        }
    }

    /// Under a decimal-comma preset the mark the person types is `,`: "0,8"
    /// is 0.8 — the core's "0.8", which their field shows as "0,8" — never 8.
    #[test]
    fn a_decimal_comma_zero_keeps_its_mark() {
        for preset in [COMMA, NumberPreset::SpaceComma] {
            assert_eq!(clean("0,", preset, Entry::Unknown, Some("0")), s("0."));
            assert_eq!(clean("0,8", preset, Entry::Unknown, Some("0,")), s("0.8"));
            assert_eq!(clean("0,8", preset, Entry::Unknown, Some("0.")), s("0.8"));
            assert_eq!(pasted("0,8", preset), s("0.8"));
            assert_eq!(clean(",", preset, Entry::Unknown, Some("")), s("0."));
            assert_eq!(
                clean("0,08", preset, Entry::Unknown, Some("0,0")),
                s("0.08")
            );
            assert_eq!(pasted("008,5", preset), s("8.5"));
        }
        // One typed comma under a decimal-point preset is the mark too.
        assert_eq!(clean("0,", DOT, Entry::Unknown, Some("0")), s("0."));
    }

    /// A pasted "0,800" is not thousands grouping under a decimal-point
    /// preset: nobody groups 800 behind a zero, and it is somebody's 0,800.
    #[test]
    fn a_zero_led_figure_is_never_read_as_grouping() {
        assert_eq!(pasted("0,800", DOT), None);
        assert_eq!(clean("0,800", DOT, Entry::Unknown, Some("")), None);
        assert_eq!(pasted("0.800.000", COMMA), None);
        assert_eq!(pasted("0,800,000", DOT), None);
        // Where the comma IS the decimal mark it reads as one.
        assert_eq!(pasted("0,800", COMMA), s("0.800"));
        // Real grouping is untouched.
        assert_eq!(pasted("1,234", DOT), s("1234"));
        assert_eq!(pasted("1.234.567", COMMA), s("1234567"));
    }

    /// What `clean` returns is a fixed point: a shell that cleaned, held the
    /// result and cleaned it again on the next render sends the same figure.
    #[test]
    fn a_cleaned_figure_cleans_to_itself() {
        let inputs = [
            "08", "00", "008.5", ".", ".5", "0.08", "0.0", "4,5", "1,234.56", "0,8", "٤٫٥",
            "$08.10", "000.000",
        ];
        for preset in [DOT, COMMA, NumberPreset::SpaceComma, NumberPreset::Indian] {
            for raw in inputs {
                for entry in [Entry::Typed, Entry::Pasted] {
                    let Some(once) = clean(raw, preset, entry, None) else {
                        continue;
                    };
                    assert_eq!(
                        clean(&once, preset, entry, None).as_deref(),
                        Some(once.as_str()),
                        "{raw} → {once} under {preset:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_paste_that_is_not_one_figure_is_refused_not_salvaged() {
        for preset in [DOT, COMMA] {
            // 1.57 is ten million times 1.5e-7.
            assert_eq!(pasted("1.5e-7", preset), None);
            assert_eq!(pasted("1e5", preset), None);
            assert_eq!(pasted("0x10", preset), None);
            assert_eq!(pasted("4.5.6", preset), None);
        }
        assert_eq!(pasted("4,5,6", COMMA), None);
        // A label beside a figure is not a letter inside one.
        assert_eq!(pasted("$4.00 USD", DOT), s("4.00"));
        assert_eq!(pasted("0.5 ETH", DOT), s("0.5"));
    }

    #[test]
    fn one_typed_comma_is_the_decimal_mark_under_a_decimal_point_preset_too() {
        assert_eq!(clean("4,", DOT, Entry::Typed, Some("4")), s("4."));
        assert_eq!(clean(",5", DOT, Entry::Typed, Some("5")), s("0.5"));
        // Not a keystroke — a paste, an autofill, a keyboard's clipboard strip
        // calling itself typing: still grouping.
        assert_eq!(clean("1,234", DOT, Entry::Typed, Some("")), s("1234"));
        assert_eq!(clean("1,234", DOT, Entry::Pasted, Some("")), s("1234"));
        assert_eq!(typed("1,234", DOT), s("1234"));
        // A figure that has its decimal point already: the comma is dropped.
        assert_eq!(clean("1,2.5", DOT, Entry::Typed, Some("12.5")), s("12.5"));
    }

    #[test]
    fn only_what_an_amount_is_written_with_is_kept() {
        assert_eq!(typed("-4", DOT), s("4"));
        assert_eq!(typed("1e5", DOT), s("15"));
        assert_eq!(typed("$4.00 ", DOT), s("4.00"));
        assert_eq!(typed("1'234.5", DOT), s("1234.5"));
    }

    #[test]
    fn arabic_indic_and_persian_digits_map_with_their_decimal_marks() {
        assert_eq!(typed("٤.٥", DOT), s("4.5"));
        assert_eq!(typed("۱۲", DOT), s("12"));
        // Digits without the mark is ten times the figure.
        assert_eq!(typed("٤٫٥", DOT), s("4.5"));
        assert_eq!(typed("٤٫٥", COMMA), s("4.5"));
        // Full-width, as a ja / zh / ko keyboard writes it.
        assert_eq!(typed("４．５", DOT), s("4.5"));
        assert_eq!(typed("4。5", DOT), s("4.5"));
        assert_eq!(typed("４，５", COMMA), s("4.5"));
        assert_eq!(pasted("１，２３４．５", DOT), s("1234.5"));
    }

    #[test]
    fn a_native_field_that_cannot_tell_reads_one_added_key_as_typing() {
        // The Android / iOS / desktop case: the decimal pad's only mark is `,`
        // while the app's preset is comma-dot.
        assert_eq!(clean("4,", DOT, Entry::Unknown, Some("4")), s("4."));
        assert_eq!(clean("4.5", DOT, Entry::Unknown, Some("4.")), s("4.5"));
        // Anything more is a paste, and a paste with no reading is refused.
        assert_eq!(clean("1.5e-7", DOT, Entry::Unknown, Some("")), None);
        assert_eq!(
            clean("1.234,56", DOT, Entry::Unknown, Some("")),
            s("1234.56")
        );
        // A key typed into a figure: "1.23456" → "1.234,56" is one key, and a
        // key never moves a figure a thousandfold.
        assert_eq!(
            clean("1.234,56", DOT, Entry::Unknown, Some("1.23456")),
            s("1.23456")
        );
        // No previous text known: nothing is a keystroke.
        assert_eq!(clean("1.5e-7", DOT, Entry::Unknown, None), None);
    }

    #[test]
    fn a_comma_that_is_neither_decimal_nor_grouping_is_refused() {
        // Arriving whole under a decimal-point preset: 15 or 150 is ten
        // times what a decimal-comma writer meant, 1.5 is a guess.
        assert_eq!(clean("1,5", DOT, Entry::Typed, Some("")), None);
        assert_eq!(pasted("1,50", DOT), None);
        assert_eq!(clean("1,5", DOT, Entry::Unknown, Some("")), None);
        // Real grouping still reads as grouping…
        assert_eq!(clean("1,234", DOT, Entry::Typed, Some("")), s("1234"));
        assert_eq!(typed("12,34,567", DOT), s("1234567"));
        // …one typed comma is still the decimal mark…
        assert_eq!(clean("1,", DOT, Entry::Typed, Some("1")), s("1."));
        // …and where the person writes a decimal comma it is one.
        assert_eq!(pasted("1,5", COMMA), s("1.5"));
    }

    #[test]
    fn grouping_only_is_thousands_or_lakhs() {
        assert!(grouping_only("1,234"));
        assert!(grouping_only("1,234,567"));
        assert!(grouping_only("12,34,567"));
        assert!(!grouping_only("1,5"));
        assert!(!grouping_only("1,50"));
        assert!(!grouping_only("1234,567"));
        assert!(!grouping_only("1,2345"));
        assert!(!grouping_only(",5"));
        assert!(!grouping_only("1,"));
    }

    #[test]
    fn grouping_is_three_digits_a_group() {
        assert!(grouped("1,234.5", ',', '.'));
        assert!(grouped("1,234,567", ',', '.'));
        assert!(!grouped("1,234", ',', '.'));
        assert!(!grouped("1,2345.5", ',', '.'));
        assert!(!grouped("1234,567,890", ',', '.'));
        assert!(!grouped("1,234.", ',', '.'));
        assert!(grouped("1.234,56", '.', ','));
    }

    #[test]
    fn the_caret_stands_after_as_many_kept_characters_as_stood_before_it() {
        // "1," alone cleans to "1." — two characters — but in the whole
        // figure the comma is dropped, and the caret belongs after the 1.
        assert_eq!(caret_after_clean("1,2.5", "12.5", 2), 1);
        assert_eq!(caret_after_clean("4.5x", "4.5", 4), 3);
        assert_eq!(caret_after_clean("4,5", "4.5", 2), 2);
        assert_eq!(caret_after_clean("1.234,56", "1234.56", 8), 7);
        assert_eq!(caret_after_clean("٤٫٥", "4.5", 3), 3);
        assert_eq!(caret_after_clean("12", "12", 0), 0);
    }

    /// Issue #421's edits, and where the caret lands after each: a native
    /// field that left it where the key was would put the next digit in the
    /// wrong place — between "0" and "." the "5" of ".5" reads 5, not 0.5.
    #[test]
    fn the_caret_follows_a_zero_given_or_taken() {
        // "." → "0.": after the mark, so the next key is a decimal digit.
        assert_eq!(caret_after_clean(".", "0.", 1), 2);
        assert_eq!(caret_after_clean(",", "0.", 1), 2);
        assert_eq!(caret_after_clean(".5", "0.5", 1), 2);
        assert_eq!(caret_after_clean(".5", "0.5", 2), 3);
        // Before the mark it stays before the zero.
        assert_eq!(caret_after_clean(".5", "0.5", 0), 0);
        // "08" → "8": after the 8.
        assert_eq!(caret_after_clean("08", "8", 2), 1);
        // A zero typed in FRONT of "8" is refused, and the caret stays put.
        assert_eq!(caret_after_clean("08", "8", 1), 0);
        assert_eq!(caret_after_clean("008.5", "8.5", 5), 3);
        assert_eq!(caret_after_clean("00", "0", 2), 1);
        // A zero that is kept is still counted.
        assert_eq!(caret_after_clean("0.5", "0.5", 1), 1);
        assert_eq!(caret_after_clean("00.5", "0.5", 4), 3);
    }

    #[test]
    fn preset_keys_are_the_shells_wire_names() {
        assert_eq!(preset_of("dot_comma"), NumberPreset::DotComma);
        assert_eq!(preset_of("space_comma"), NumberPreset::SpaceComma);
        assert_eq!(preset_of("indian"), NumberPreset::Indian);
        assert_eq!(preset_of("comma_dot"), NumberPreset::CommaDot);
        assert_eq!(preset_of("auto"), NumberPreset::CommaDot);
    }
}
